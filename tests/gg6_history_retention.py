# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Real TCP replay and acceptance with a full synthetic terminal history.

The fixture bypasses acceptance only to reach the existing retention limit
without waiting days. This is a functional regression, not an R0/soak result.
"""
import argparse
import asyncio
from contextlib import closing
import hashlib
import json
from pathlib import Path
import re
import sqlite3
import tempfile
import time

from lifecycle import Server
from r0_workload import Wire, metric_snapshot, prepare

HISTORY = 100000
PENDING = 200


def fixture(data):
    now = int(time.time())
    expected = {}

    def messages():
        for index in range(HISTORY + PENDING):
            sequence = index + 1
            pending = index >= HISTORY
            recipient = 5 + index % 20 if pending else 2
            text = ('rgg-history-%010d' % sequence).encode().ljust(128, b'x')
            if pending:
                expected[sequence] = dict(sender=1, recipient=recipient, text=text, offline=True)
            yield (sequence, 1, recipient, sequence, 1, 8, text.decode() if pending else None,
                   '[]' if pending else None, '[]' if pending else None, 132, now, now + 86400)

    # Connection.__exit__ commits but does not close; an unclosed fixture keeps
    # a 14 MB WAL beside the database until GC, exhausting the 32 MiB test tmpfs.
    with closing(sqlite3.connect(data / 'rgg.sqlite')) as db, db:
        db.execute('PRAGMA foreign_keys=ON')
        db.executemany('INSERT INTO messages(id,sender,recipient,client_sequence,payload_version,presentation,body,styles,conference,payload_bytes,accepted_at,expires_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)', messages())
        db.executemany('INSERT INTO deliveries(message_id,recipient,enqueue_order,state,terminal_at) VALUES(?,?,?,?,?)',
                       ((i + 1, 5 + i % 20 if i >= HISTORY else 2, i + 1,
                         'Pending' if i >= HISTORY else 'TransportWrittenUnconfirmed',
                         None if i >= HISTORY else now) for i in range(HISTORY + PENDING)))
        assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
        assert db.execute('PRAGMA foreign_key_check').fetchall() == []
    return expected


async def replay(port, metrics, expected):
    clients, errors = [], []
    started = time.monotonic()
    loop = asyncio.get_running_loop()

    async def snapshot():
        return await loop.run_in_executor(None, metric_snapshot, '127.0.0.1', metrics)

    async def finished():
        deadline = time.monotonic() + 45
        while any(not value.get('received') for value in expected.values()):
            assert not errors, errors[:3]
            assert time.monotonic() < deadline, 'full-history replay deadline'
            await asyncio.sleep(.02)
        deadline = time.monotonic() + 8
        while (await snapshot())['rgg_queued_payload_bytes'] != 0:
            assert not errors and time.monotonic() < deadline, 'terminal writes did not finish'
            await asyncio.sleep(.05)

    try:
        for user in range(5, 25):
            clients.append(await Wire.connect('127.0.0.1', port, user, expected, errors))
        await finished()
        for client in clients:
            assert client.received == [seq for seq, item in expected.items() if item['recipient'] == client.user]
        replay_seconds = time.monotonic() - started
        # Acceptance must still finish with full history, then survive an offline interval.
        sender = await Wire.connect('127.0.0.1', port, 1, expected, errors)
        clients.append(sender)
        sequence = HISTORY + PENDING + 1
        expected[sequence] = dict(sender=1, recipient=2, text=b'after full-history replay', offline=True)
        await sender.send(sequence, time.monotonic())
        clients.append(await Wire.connect('127.0.0.1', port, 2, expected, errors))
        await finished()
        values = await snapshot()
        assert not errors, errors[:3]
        assert values['rgg_delivery_records{state="leased"}'] == 0
        assert values['rgg_operations_total{operation="delivery",result="timeout"}'] == 0
        return dict(replay_seconds=round(replay_seconds, 3), total_seconds=round(time.monotonic() - started, 3),
                    new_acceptance_ack_ms=round((expected[sequence]['ack'] - expected[sequence]['sent']) * 1000, 3))
    finally:
        await asyncio.gather(*(client.close() for client in clients), return_exceptions=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='rgg-full-history-') as temporary:
        root = Path(temporary)
        data = root / 'data'
        prepare(args.admin.resolve(), data, 24)
        expected = fixture(data)
        config = root / 'rgg.toml'
        config.write_text("data_dir='data'\nhttp_bind='127.0.0.1:0'\ngg_bind='127.0.0.1:0'\nmetrics_bind='127.0.0.1:0'\n[network]\nmax_connections=32\nper_ip=32\n", encoding='utf-8')
        server = Server(args.server.resolve(), config, root / 'server.log')
        try:
            server.ready()
            log = server.log.read_text()
            port = int(re.search(r'gg_endpoint=127\.0\.0\.1:(\d+)', log).group(1))
            metrics = int(re.search(r'metrics_endpoint=127\.0\.0\.1:(\d+)', log).group(1))
            report = asyncio.run(replay(port, metrics, expected))
        finally:
            server.close()
        assert server.process.returncode == 0, server.log.read_text()
        # Each transition clears payloads atomically. Cardinality pruning runs during
        # acceptance/claim/maintenance, so final transitions can temporarily exceed the cap.
        with closing(sqlite3.connect(data / 'rgg.sqlite')) as db, db:
            assert db.execute('SELECT count(*) FROM account_sessions').fetchone()[0] == 0
            assert db.execute('SELECT count(*) FROM deliveries WHERE state!="TransportWrittenUnconfirmed"').fetchone()[0] == 0
            assert db.execute('SELECT count(*) FROM messages WHERE body IS NOT NULL OR styles IS NOT NULL OR conference IS NOT NULL').fetchone()[0] == 0
        server = Server(args.server.resolve(), config, root / 'restart.log')
        try:
            server.ready()
            deadline = time.monotonic() + 8
            while True:
                with closing(sqlite3.connect(data / 'rgg.sqlite')) as db, db:
                    total = db.execute('SELECT count(*) FROM deliveries').fetchone()[0]
                if total == HISTORY:
                    break
                assert time.monotonic() < deadline, 'maintenance did not trim history'
                time.sleep(.05)
        finally:
            server.close()
        assert server.process.returncode == 0
        with closing(sqlite3.connect(data / 'rgg.sqlite')) as db, db:
            assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
            assert db.execute('PRAGMA foreign_key_check').fetchall() == []
            assert db.execute('SELECT min(id),max(id),count(*) FROM messages').fetchone() == (PENDING + 2, HISTORY + PENDING + 1, HISTORY)
            assert db.execute('SELECT count(*) FROM delivery_attempts WHERE outcome IS NULL').fetchone()[0] == 0
    report.update(passed=True, history=HISTORY, offline=PENDING, receivers=20, restart_retention_verified=True,
                  server_sha256=hashlib.sha256(args.server.read_bytes()).hexdigest(), capacity_qualified=False)
    print('PASS: full history ' + json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
