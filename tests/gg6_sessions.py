# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Independent wire encoding, actual TCP/processes and disposable data; no native-client claim.
GG32 oracle is imported from the attributed poggadaj port in tools/gg32_vectors.py.
All accounts, messages and passwords are synthetic. No production endpoints are used.
"""
import argparse
import contextlib
import json
import os
from pathlib import Path
import re
import socket
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time
from lifecycle import Server
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from gg32_vectors import oracle

PASSWORD = b"LocalTest9!"
def words(*items): return struct.pack("<" + "I" * len(items), *items)
def until(check, seconds=5):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if check(): return
        time.sleep(.02)
    raise AssertionError("condition deadline exceeded")

class Client:
    def __init__(self, address, small=False):
        self.socket = socket.socket()
        if small: self.socket.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 1024)
        self.socket.settimeout(3)
        self.socket.connect(address)
        kind, body = self.receive()
        assert kind == 1 and len(body) == 4
        self.seed, = struct.unpack("<I", body)
    def close(self): self.socket.close()
    def send(self, kind, body=b""): self.socket.sendall(words(kind, len(body)) + body)
    def exact(self, size):
        result = b""
        while len(result) < size:
            part = self.socket.recv(size - len(result))
            if not part: raise EOFError()
            result += part
        return result
    def receive(self):
        kind, size = struct.unpack("<II", self.exact(8))
        assert size <= 65536
        return kind, self.exact(size)
    def login(self, user, password=PASSWORD, response=None, ready=True):
        value = oracle(password, self.seed) if response is None else response
        self.send(0x15, words(user, value, 2, 0x20) + bytes(13) + bytes([255, 0xbe]))
        result = self.receive()
        if result == (3, b"") and ready: self.send(0x12)
        return result
    def ping(self): self.send(8); assert self.receive() == (7, b"")
    def message(self, recipient, seq, text="Żółć", flags=8, options=b"", ack=True):
        self.send(0xb, words(recipient, seq, flags) + text.encode("cp1250") + b"\0" + options)
        if ack:
            kind, body = self.receive(); assert kind == 5, (kind, body)
            status, target, sequence = struct.unpack("<III", body)
            assert (target, sequence) == (recipient, seq)
            return status
    def quiet(self, seconds=.15):
        self.socket.settimeout(seconds)
        try:
            self.receive()
            raise AssertionError("unexpected frame")
        except socket.timeout: pass
        finally: self.socket.settimeout(3)
    def closed(self):
        try:
            frame = self.receive()
            assert frame == (0xb, b""), frame
            assert self.socket.recv(1) == b""
        except (EOFError, ConnectionResetError): pass

class Lab:
    def __init__(self, binary, admin, root):
        self.binary, self.admin, self.root = binary, admin, root
        self.data = root / "data"
        self.server = None
        self.clients = []
        self.run("init")
        code = self.run("invite")["codes"][0]
        for user in [1, 2, 3]:
            self.run("account-create", payload={"requested_uin": user, "invite": code, "password": PASSWORD.decode(), "email": "synthetic@example.invalid"})
    def run(self, command, *args, payload=None):
        result = subprocess.run([str(self.admin), command, "--data-dir", str(self.data), *args], input=json.dumps(payload).encode() if payload else None, capture_output=True, timeout=10)
        assert result.returncode == 0, result.stderr
        return json.loads(result.stdout)
    def start(self, extra="", fault=None, drop_wake=False, discovery=False, registration=False, statistics=False, downloads=None, metrics=False):
        config = self.root / "rgg.toml"
        config.write_text("data_dir='data'\nhttp_bind='127.0.0.1:0'\ngg_bind='127.0.0.1:0'\nlegacy_discovery="+str(discovery).lower()+"\nweb_registration="+str(registration).lower()+"\nweb_public_stats="+str(statistics).lower()+("\ndownloads_manifest='"+str(downloads)+"'" if downloads is not None else "")+("\nmetrics_bind='127.0.0.1:0'" if metrics else "")+"\n[network]\nlogin_seconds=1\ninitial_seconds=1\nwrite_millis=500\n" + extra, encoding="utf-8")
        env = os.environ.copy()
        env.pop("RGG_TEST_CRASH", None)
        env.pop("RGG_TEST_DROP_WAKEUPS", None)
        if fault: env["RGG_TEST_CRASH"] = fault
        if drop_wake: env["RGG_TEST_DROP_WAKEUPS"] = "1"
        self.server = Server(self.binary, config, self.root / "server.log", env=env)
        self.server.ready()
        until(lambda: re.search(r"gg_endpoint=127\.0\.0\.1:(\d+)", self.server.log.read_text()))
        self.address = ("127.0.0.1", int(re.search(r"gg_endpoint=127\.0\.0\.1:(\d+)", self.server.log.read_text()).group(1)))
    def client(self, user=None, ready=True, small=False):
        c = Client(self.address, small=small); self.clients.append(c)
        if user is not None: assert c.login(user, ready=ready) == (3, b"")
        return c
    def stop(self, hard=False):
        if self.server:
            self.server.close(hard=hard)
            assert PASSWORD not in self.server.log.read_bytes()
            assert "Żółć" not in self.server.log.read_text()
        for c in self.clients: c.close()
        self.clients = []
    def sql(self, query):
        with sqlite3.connect(str(self.data / "rgg.sqlite")) as db: return db.execute(query).fetchall()

def normal(lab):
    lab.start()
    c = lab.client(); assert c.login(1, password=b"Incorrect9!") == (9, b""); c.closed(); c.close()
    c = lab.client(); assert c.login(999) == (9, b""); c.close()
    c = lab.client(); saved = oracle(PASSWORD, c.seed); c.close()
    c = lab.client(); assert c.login(1, response=saved) == (9, b""); c.close()
    c = lab.client(); c.send(8); assert c.receive() == (9, b""); c.close()
    c = lab.client(); assert c.receive() == (9, b""); c.closed(); c.close()  # handshake timeout
    a = lab.client(1); a.ping()
    style = bytes([2, 6, 0, 0, 0, 9, 255, 0, 127])
    for seq, text, options in [(7, "Żółć", style), (7, "powtórzony numer", b""), (8, "trzecia", b"")]:
        assert a.message(2, seq, text, flags=9, options=options) == 3
        assert lab.sql("SELECT count(*) FROM deliveries")[0][0] >= 1  # observable commit BEFORE ACK
    a.message(2, 9, "bez ACK", flags=40, ack=False); a.ping()
    assert a.message(2, 10, options=bytes([1])) == 6  # truncated conference option
    assert a.message(2, 10, options=bytes([2, 3, 0, 0, 0, 128])) == 6  # unsupported image
    assert a.message(999, 11) == 6
    assert lab.sql("SELECT count(*) FROM messages")[0][0] == 4
    lab.stop(hard=True); lab.start(drop_wake=True)
    b = lab.client(2, ready=False); b.quiet()
    assert lab.sql("SELECT count(*) FROM deliveries WHERE state='Pending'")[0][0] == 4
    b.send(0x12)
    for seq, text, options in [(7, "Żółć", style), (7, "powtórzony numer", b""), (8, "trzecia", b""), (9, "bez ACK", b"")]:
        kind, body = b.receive(); assert kind == 10
        sender, actual_seq, stamp, flags = struct.unpack("<IIII", body[:16])
        assert sender == 1 and seq == actual_seq and flags == 9 and stamp > 0
        assert body[16:] == text.encode("cp1250") + b"\0" + options
    b.ping(); until(lambda: lab.sql("SELECT count(*) FROM messages WHERE body IS NOT NULL")[0][0] == 0)
    a = lab.client(1); assert a.message(2, 20, "online") == 3
    kind, body = b.receive(); assert kind == 10 and struct.unpack("<I", body[12:16])[0] == 8  # polling recovers missing wakeup
    newer = lab.client(2); b.closed(); newer.ping(); a.ping()
    newer.send(0x15, words(2, oracle(PASSWORD, newer.seed), 2, 0x20) + bytes(15)); newer.closed()
    last = lab.client(2); last.ping()
    lab.server.close(); last.closed()  # normal FIN, no forced-disconnect packet
    assert lab.server.process.returncode == 0
    lab.stop(); lab.start(); b = lab.client(2); b.quiet(); b.ping()
    assert lab.sql("PRAGMA integrity_check") == [("ok",)]

def privacy(lab):
    lab.start(); a = lab.client(1); assert a.message(2, 1) == 3
    b = lab.client(2, ready=False); b.send(0xf, words(1) + bytes([4])); b.quiet(); assert lab.sql("SELECT count(*) FROM deliveries WHERE state='Pending'") == [(1,)]
    b.send(0x10); b.ping(); b.quiet()
    assert lab.sql("SELECT state FROM deliveries") == [("RejectedByPrivacy",)]
    assert a.message(2, 2) == 6
    b.send(0xe, words(1) + bytes([1])); b.ping(); assert a.message(2, 3) == 6
    b.close(); lab.stop(hard=True); lab.start(); a = lab.client(1)
    assert a.message(2, 4) == 6  # saved offline block survives restart
    b = lab.client(2, ready=False); b.send(0xf); b.closed()  # incomplete list never clears policy
    assert a.message(2, 5) == 6
    b = lab.client(2); b.ping(); b.quiet(); assert a.message(2, 6) == 3; assert b.receive()[0] == 10
    c = lab.client(3, ready=False); c.message(2, 7, ack=False); c.closed()  # no send before Ready

def quotas(lab):
    lab.start(extra="[queue]\nper_recipient_messages=2\n")
    a = lab.client(1)
    assert [a.message(2, n) for n in range(3)] == [3, 3, 4]
    assert lab.sql("SELECT count(*) FROM messages") == [(2,)]
    a.send(0xb, bytes(4097)); a.closed()  # rejects declared size before buffering payload
    c = lab.client(3); c.ping()

def connection_limits(lab):
    lab.start(extra="max_connections=2\nper_ip=2\n")
    a = lab.client(1); b = lab.client(2)
    with socket.create_connection(lab.address, timeout=2) as rejected:
        assert rejected.recv(1) == b""
    a.ping(); b.ping(); a.close()
    until(lambda: lab.sql("SELECT count(*) FROM account_sessions WHERE uin=1") == [(0,)])
    c = lab.client(3); c.ping()
    c.socket.sendall(words(0xb, 0xffffffff)); c.closed()

def slow_receiver(lab):
    lab.start(); a = lab.client(1); b = lab.client(2, small=True)
    for n in range(180): assert a.message(2, n, "x" * 2000) == 3
    a.ping()
    until(lambda: lab.sql("SELECT count(*) FROM account_sessions WHERE uin=2") == [(0,)], seconds=5)
    assert lab.sql("SELECT count(*) FROM deliveries WHERE state='Leased'") == [(0,)]
    assert lab.sql("SELECT count(*) FROM deliveries WHERE state='Pending'")[0][0] > 0
    c = lab.client(2)
    remaining = lab.sql("SELECT count(*) FROM deliveries WHERE state='Pending'")[0][0]
    # Count all wire frames until a ping barrier (some already became terminal before the SQL read).
    time.sleep(.2); c.send(8); received = 0
    while True:
        kind, body = c.receive()
        if kind == 7: break
        assert kind == 10; received += 1
    assert received > 0 or remaining == 0
    a.ping()

def crashes(lab, stage):
    lab.start(fault=stage); a = lab.client(1)
    if stage in ("before_accept_commit", "after_accept_commit"):
        a.message(2, 0xfeed, "awaria", ack=False)
    else:
        assert a.message(2, 0xfeed, "awaria") == 3
        b = lab.client(2)
        if stage == "after_delivery_write": assert b.receive()[0] == 10
    lab.server.process.wait(timeout=5); assert lab.server.process.returncode == 86
    lab.stop(); lab.start(); b = lab.client(2)
    if stage == "before_accept_commit":
        b.quiet(); assert lab.sql("SELECT count(*) FROM messages") == [(0,)]
    else:
        kind, body = b.receive(); assert kind == 10 and body[16:] == b"awaria\0"
        b.ping(); b.quiet(); until(lambda: lab.sql("SELECT count(*) FROM deliveries WHERE state='TransportWrittenUnconfirmed'") == [(1,)])
    assert lab.sql("PRAGMA integrity_check") == [("ok",)]

def main():
    p = argparse.ArgumentParser(); p.add_argument("--server", type=Path, required=True); p.add_argument("--admin", type=Path, required=True); p.add_argument("--faults", action="store_true"); args = p.parse_args()
    cases = [("login/sessions/FIFO/restart/encoding", normal), ("privacy readiness/persistence", privacy), ("quotas/packet bounds", quotas), ("connection limits", connection_limits), ("slow receiver isolation", slow_receiver)]
    if args.faults:
        for stage in ["before_accept_commit", "after_accept_commit", "before_delivery_write", "after_delivery_write"]:
            cases.append(("crash " + stage, lambda lab, stage=stage: crashes(lab, stage)))
    for name, test in cases:
        with tempfile.TemporaryDirectory(prefix="rgg-wire-") as root:
            lab = Lab(args.server.resolve(), args.admin.resolve(), Path(root))
            try: test(lab); print("PASS:", name, flush=True)
            finally: lab.stop()
    print("PASS: GG6 synthetic protocol suite; native GG remains unqualified", flush=True)

if __name__ == "__main__": main()
