# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Bounded real HTTP sockets, slow headers/downloads and separate GG availability (loopback)."""
import argparse
import socket
import tempfile
import time
from pathlib import Path
from gg6_sessions import Lab
from http_discovery import request
from http_downloads import fixture


def connect(lab, source='127.0.0.1', small=False):
    host, port = lab.server.address.split(':')
    sock = socket.socket()
    if small:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 1024)
    sock.settimeout(3)
    sock.bind((source, 0))
    sock.connect((host, int(port)))
    return sock


def assert_closed(sock):
    try:
        assert sock.recv(4096) == b''
    except ConnectionResetError:
        pass


def connection_and_idle_limits(lab):
    lab.start(extra='[http]\nmax_connections=2\nper_ip=1\nidle_seconds=2\nwrite_seconds=1\nlifetime_seconds=4\n')
    time.sleep(.1)  # Previous readiness socket closes before the per-IP test.
    sockets = []
    try:
        a = connect(lab); sockets.append(a)
        a.sendall(b'GET /health/live HTTP/1.1\r\n')
        time.sleep(.05)
        with connect(lab) as refused:
            # Request headers cannot create an additional TCP admission budget.
            refused.sendall(b'GET /health/live HTTP/1.1\r\nX-Forwarded-For: 203.0.113.1\r\n\r\n')
            assert_closed(refused)
        b = connect(lab, '127.0.0.2'); sockets.append(b)
        b.sendall(b'GET /health/live HTTP/1.1\r\n')
        time.sleep(.05)
        with connect(lab, '127.0.0.3') as refused:
            assert_closed(refused)  # Different peer still hits the global limit.
        lab.client(1).ping()
        time.sleep(2.1)
        for sock in sockets:
            assert_closed(sock)
        assert request(lab, '/health/ready')[0] == 200
        lab.client(2).ping()
    finally:
        for sock in sockets:
            sock.close()


def traffic_cannot_extend_lifetime(lab):
    lab.start(extra='[http]\nidle_seconds=2\nwrite_seconds=1\nlifetime_seconds=4\n')
    gg = lab.client(1)
    with connect(lab) as sock:
        sock.sendall(b'GET /health/live HTTP/1.1\r\nX-Slow: ')
        sock.settimeout(.01)
        start = time.monotonic()
        closed = False
        while time.monotonic() - start < 6:
            try:
                sock.sendall(b'x')
                data = sock.recv(1)
                assert data == b''
                closed = True
                break
            except socket.timeout:
                pass
            except (BrokenPipeError, ConnectionResetError):
                closed = True
                break
            gg.ping()
            time.sleep(.2)
        assert closed and 3 <= time.monotonic() - start < 6
    assert request(lab, '/health/ready')[0] == 200


def stalled_downloads_release_all_capacity(lab):
    fixture(lab.root, content=b'Z' * (16 * 1024 * 1024))
    lab.start(downloads='artifacts/manifest.toml', extra='[http]\nmax_connections=4\nper_ip=4\nidle_seconds=3\nwrite_seconds=1\nlifetime_seconds=8\n')
    time.sleep(.1)
    sockets = []
    try:
        for _ in range(4):
            sock = connect(lab, small=True); sockets.append(sock)
            sock.sendall(('GET /downloads/client HTTP/1.1\r\nHost: ' + lab.server.address + '\r\n\r\n').encode())
            header = b''
            while b'\r\n\r\n' not in header:
                chunk = sock.recv(1)
                assert chunk
                header += chunk
            assert header.startswith(b'HTTP/1.1 200')
        # Bodies remain unread. Do not close these clients to release their slots.
        lab.client(1).ping()
        time.sleep(1.8)
        assert request(lab, '/downloads/client', method='HEAD')[0] == 200
        assert request(lab, '/health/ready')[0] == 200
        lab.client(2).ping()
        # Cancellation/drain also terminates a new stalled download promptly.
        extra = connect(lab, small=True); sockets.append(extra)
        extra.sendall(('GET /downloads/client HTTP/1.1\r\nHost: ' + lab.server.address + '\r\n\r\n').encode())
        start = time.monotonic(); lab.stop()
        assert time.monotonic() - start < 7
    finally:
        for sock in sockets:
            sock.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    args = parser.parse_args()
    for test in [connection_and_idle_limits, traffic_cannot_extend_lifetime, stalled_downloads_release_all_capacity]:
        with tempfile.TemporaryDirectory(prefix='rgg-http-limits-') as tmp:
            lab = Lab(args.server.resolve(), args.admin.resolve(), Path(tmp))
            try:
                test(lab)
                print('PASS:', test.__name__, flush=True)
            finally:
                lab.stop()


if __name__ == '__main__':
    main()
