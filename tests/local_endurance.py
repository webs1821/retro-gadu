# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Optional bounded Linux/WSL endurance probe; synthetic data, loopback, no capacity claim."""
import argparse
import collections
import hashlib
import json
import os
import platform
import socket
import subprocess
import tempfile
import threading
import time
from pathlib import Path
from gg6_sessions import Lab, until
from http_discovery import request
from http_downloads import fixture


def process_sample(pid):
    root = Path('/proc') / str(pid)
    fields = {}
    for line in (root / 'status').read_text().splitlines():
        if line.startswith(('VmRSS:', 'VmHWM:', 'Threads:')):
            key, value = line.split(':', 1)
            fields[key] = int(value.split()[0])
    fields['fds'] = len(list((root / 'fd').iterdir()))
    stat = (root / 'stat').read_text().rsplit(')', 1)[1].split()
    fields['cpu_seconds'] = (int(stat[11]) + int(stat[12])) / os.sysconf('SC_CLK_TCK')
    return fields


def probe(args, root):
    lab = Lab(args.server, args.admin, root)
    fixture(root, content=b'Q' * (1024 * 1024))
    stop = threading.Event()
    errors, http_counts = [], collections.Counter()
    count_lock = threading.Lock()
    workers, samples, latencies = [], [], []
    messages = cycles = 0
    def http_worker():
        try:
            while not stop.is_set():
                assert request(lab, '/health/ready')[0] == 200
                code, _, content = request(lab, '/downloads/client')
                assert code in (200, 429)
                if code == 200:
                    assert content == b'Q' * (1024 * 1024)
                with count_lock:
                    http_counts[str(code)] += 1
                stop.wait(.5)
        except BaseException as error:
            errors.append(type(error).__name__)
            stop.set()
    try:
        lab.start(downloads='artifacts/manifest.toml', extra='[http]\nmax_connections=8\nper_ip=8\nidle_seconds=1\nwrite_seconds=1\nlifetime_seconds=4\n')
        sender = lab.client(1)
        baseline = process_sample(lab.server.process.pid)
        start = time.monotonic()
        for _ in range(2):
            worker = threading.Thread(target=http_worker, daemon=True)
            workers.append(worker); worker.start()
        next_notice = 30
        while time.monotonic() - start < args.seconds and not errors:
            cycle_start = time.monotonic()
            until(lambda: lab.sql('SELECT count(*) FROM account_sessions WHERE uin=2') == [(0,)])
            receiver = lab.client(2) if cycles % 2 else None
            texts = ['rGG synthetic batch {} item {} Żółć'.format(cycles, i) for i in range(25)]
            for text in texts:
                began = time.monotonic()
                assert sender.message(2, messages + 1, text) == 3
                latencies.append((time.monotonic() - began) * 1000)
                messages += 1
            if receiver is None:
                receiver = lab.client(2)
            for text in texts:
                kind, payload = receiver.receive()
                assert kind == 10 and payload[16:] == text.encode('cp1250') + b'\0'
            receiver.ping(); receiver.close()
            until(lambda: lab.sql('SELECT count(*) FROM messages WHERE body IS NOT NULL') == [(0,)])
            if cycles % 5 == 0:
                # Partial headers consume an HTTP slot briefly, then expire without client closure.
                host, port = lab.server.address.split(':')
                with socket.create_connection((host, int(port)), timeout=3) as stalled:
                    stalled.sendall(b'GET /health/live HTTP/1.1\r\nX-Incomplete: ')
                    try:
                        assert stalled.recv(1) == b''
                    except ConnectionResetError:
                        pass
                sender.ping()
            sample = process_sample(lab.server.process.pid)
            sample['elapsed_seconds'] = round(time.monotonic() - start, 3)
            samples.append(sample)
            assert sample['VmHWM'] <= args.rss_limit_mib * 1024, 'RSS safety envelope exceeded'
            assert sample['Threads'] <= baseline['Threads'] + 8, 'unexpected thread growth'
            cycles += 1
            if sample['elapsed_seconds'] >= next_notice:
                print('PROGRESS: {:.0f}s, {} messages verified, RSS {:.1f} MiB'.format(sample['elapsed_seconds'], messages, sample['VmRSS']/1024), flush=True)
                next_notice += 30
            stop.wait(max(0, 1 - (time.monotonic() - cycle_start)))
        stop.set()
        for worker in workers:
            worker.join(timeout=5)
            assert not worker.is_alive()
        assert not errors, errors
        assert lab.sql('PRAGMA integrity_check') == [('ok',)] and lab.sql('PRAGMA foreign_key_check') == []
        assert lab.sql('SELECT count(*) FROM messages WHERE body IS NOT NULL') == [(0,)]
        assert request(lab, '/health/ready')[0] == 200
        elapsed = time.monotonic() - start
        final = process_sample(lab.server.process.pid)
        latencies.sort()
        report = dict(format=1, passed=True, scope='synthetic loopback probe; not production sizing or long soak',
                      version=subprocess.check_output([str(args.server), '--version'], text=True).strip(),
                      binary_sha256=hashlib.sha256(args.server.read_bytes()).hexdigest(),
                      platform=platform.system() + ' ' + platform.machine(), duration_seconds=round(elapsed, 3),
                      configured_rss_envelope_mib=args.rss_limit_mib, cycles=cycles, messages_verified=messages,
                      http_responses=dict(http_counts), baseline=baseline, final=final,
                      ack_ms_p50=round(latencies[len(latencies)//2], 3),
                      ack_ms_p95=round(latencies[int((len(latencies)-1)*.95)], 3), samples=samples,
                      cpu_seconds=round(final['cpu_seconds']-baseline['cpu_seconds'], 3))
        lab.stop()
        assert lab.server.process.returncode == 0
        report['graceful_shutdown'] = True
        return report
    finally:
        stop.set()
        for worker in workers:
            worker.join(timeout=5)
        lab.stop()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=180)
    parser.add_argument('--rss-limit-mib', type=int, default=256)
    args = parser.parse_args()
    assert platform.system() == 'Linux' and 5 <= args.seconds <= 3600 and 32 <= args.rss_limit_mib <= 1024
    assert not args.report.exists(), 'report must be new'
    args.server = args.server.resolve(); args.admin = args.admin.resolve()
    with tempfile.TemporaryDirectory(prefix='rgg-endurance-') as tmp:
        report = probe(args, Path(tmp))
    with args.report.open('x', encoding='utf-8') as output:
        json.dump(report, output, indent=2); output.write('\n')
    print('PASS: {} verified online/offline messages, {} cycles, peak RSS {:.1f} MiB, clean shutdown'.format(report['messages_verified'], report['cycles'], report['final']['VmHWM']/1024), flush=True)


if __name__ == '__main__':
    main()
