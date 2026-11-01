# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Concurrent replay beside terminal history; synthetic offline fixture, real GG6 TCP.

The fixture deliberately bypasses acceptance to test existing storage independently.
The separate R0 workload verifies acceptance and replay together through TCP.
"""
import argparse
import asyncio
import json
from pathlib import Path
import re
import sqlite3
import tempfile
import time
from lifecycle import Server
from r0_workload import Wire, metric_snapshot, prepare


def fixture(data):
    now = int(time.time())
    expected, messages, deliveries = {}, [], []
    for index in range(6000):
        sequence = index + 1
        sender = index % 4 + 1
        pending = index >= 3000
        recipient = 5 + index % 20 if pending else sender % 4 + 1
        text = ('rgg-replay-%010d:' % sequence).encode().ljust(128, b'x')
        messages.append((sequence, sender, recipient, sequence, 1, 8,
                         text.decode() if pending else None, '[]' if pending else None,
                         '[]' if pending else None, 132, now, now + 86400))
        deliveries.append((sequence, recipient, sequence,
                           'Pending' if pending else 'TransportWrittenUnconfirmed',
                           None if pending else now))
        if pending:
            expected[sequence] = dict(sender=sender, recipient=recipient, text=text, offline=True)
    with sqlite3.connect(str(data / 'rgg.sqlite')) as db:
        db.execute('PRAGMA foreign_keys=ON')
        db.executemany('INSERT INTO messages(id,sender,recipient,client_sequence,payload_version,presentation,body,styles,conference,payload_bytes,accepted_at,expires_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)', messages)
        db.executemany('INSERT INTO deliveries(message_id,recipient,enqueue_order,state,terminal_at) VALUES(?,?,?,?,?)', deliveries)
        assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
        assert db.execute('PRAGMA foreign_key_check').fetchall() == []
    return expected


async def replay(gg, metrics, expected):
    errors, clients = [], []
    started = time.monotonic()
    loop = asyncio.get_running_loop()
    async def snapshot():
        return await loop.run_in_executor(None, metric_snapshot, '127.0.0.1', metrics)
    try:
        for user in range(5, 25):
            clients.append(await Wire.connect('127.0.0.1', gg, user, expected, errors))
        # Functional regression deadline; R0 has its own stricter capacity gates.
        deadline = started + 60
        while any(not value.get('received') for value in expected.values()):
            assert not errors, errors[:3]
            assert time.monotonic() < deadline, 'concurrent replay did not finish'
            await asyncio.sleep(.02)
        for client in clients:
            assert client.received == [seq for seq, item in expected.items() if item['recipient'] == client.user], 'FIFO mismatch'
        deadline = time.monotonic() + 8
        while True:
            state = await snapshot()
            if state['rgg_queued_payload_bytes'] == 0 and state['rgg_delivery_records{state="written_unconfirmed"}'] == 6000:
                break
            assert not errors and time.monotonic() < deadline, 'writes did not finish after TCP delivery'
            await asyncio.sleep(.1)
        assert not errors, errors[:3]
        assert state['rgg_sessions{phase="ready"}'] == 20
        assert state['rgg_delivery_records{state="leased"}'] == 0
        assert state['rgg_operations_total{operation="delivery",result="timeout"}'] == 0
        return dict(passed=True,history=3000,offline=3000,receivers=20,
                    seconds=round(time.monotonic()-started,3),capacity_qualified=False)
    finally:
        await asyncio.gather(*(client.close() for client in clients), return_exceptions=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='rgg-replay-history-') as tmp:
        root = Path(tmp); data = root / 'data'
        prepare(args.admin.resolve(), data, 24)
        expected = fixture(data)
        config = root / 'rgg.toml'
        config.write_text("data_dir='data'\nhttp_bind='127.0.0.1:0'\ngg_bind='127.0.0.1:0'\nmetrics_bind='127.0.0.1:0'\n[network]\nmax_connections=32\nper_ip=32\n", encoding='utf-8')
        server = Server(args.server.resolve(), config, root / 'server.log')
        try:
            server.ready(); log = server.log.read_text()
            gg = int(re.search(r'gg_endpoint=127\.0\.0\.1:(\d+)', log).group(1))
            metrics = int(re.search(r'metrics_endpoint=127\.0\.0\.1:(\d+)', log).group(1))
            report = asyncio.run(replay(gg, metrics, expected))
        finally:
            server.close()
        assert server.process.returncode == 0, server.log.read_text()
        with sqlite3.connect(str(data / 'rgg.sqlite')) as db:
            assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert db.execute('PRAGMA foreign_key_check').fetchall() == []
            assert db.execute('SELECT count(*) FROM account_sessions').fetchone()[0] == 0
            assert db.execute('SELECT count(*) FROM messages WHERE body IS NOT NULL').fetchone()[0] == 0
            assert dict(db.execute('SELECT state,count(*) FROM deliveries GROUP BY state')) == {'TransportWrittenUnconfirmed': 6000}
        print('PASS: concurrent replay '+json.dumps(report), flush=True)


if __name__ == '__main__': main()
