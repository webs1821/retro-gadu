# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Independent GG6 wire tests for presence, disclosure and versioned contact archives.
Disposable synthetic data only; imports the existing explicitly attributed GG32 oracle.
"""
import argparse
from pathlib import Path
import socket
import struct
import tempfile
import time
from gg6_sessions import Lab, words, PASSWORD, oracle, until

def take(c, kind, predicate=lambda b: True):
    mail = getattr(c, "mail", [])
    c.mail = mail
    for i, (k, b) in enumerate(mail):
        if k == kind and predicate(b): return mail.pop(i)[1]
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        k, b = c.receive()
        if k == kind and predicate(b): return b
        mail.append((k, b))
    raise AssertionError("packet deadline")

def barrier(c): c.send(8); assert take(c, 7) == b""
def rules(c, entries):
    c.send(0x10, b"".join(words(n) + bytes([flags]) for n, flags in entries))
    expected = sum(bool(flags & 1) for _, flags in entries)
    found = []
    while len(found) < expected: found.extend(parse_status(take(c, 0x11), True))
    barrier(c)
    return found

def parse_status(body, sized):
    rows = []
    while body:
        assert len(body) >= 14
        user, = struct.unpack("<I", body[:4]); state = body[4]
        assert body[5:12] == bytes(7) and body[13] == 0, "unexpected metadata disclosure"
        assert body[12] == (255 if state in (2,3,4,5) else 0), "unexpected image capability"
        body = body[14:]; text = ""; stamp = None
        if state in (4, 5, 0x15):
            if sized:
                size = body[0]; tail, body = body[1:1+size], body[1+size:]
            else: tail, body = body, b""
            value, nul, rest = tail.partition(b"\0")
            assert nul and len(rest) in (0, 4)
            text = value.decode("cp1250")
            if rest: stamp, = struct.unpack("<I", rest)
        rows.append((user, state, text, stamp))
        if not sized: assert not body
    return rows

def status(c, value, text=None, stamp=None):
    body = words(value)
    if text is not None:
        body += text.encode("cp1250") + b"\0"
        if stamp is not None: body += words(stamp)
    c.send(2, body)

def change(c, expected):
    take(c, 0xf, lambda b: parse_status(b, False) == [expected])

def visibility(lab):
    lab.start(); a = lab.client(1); b = lab.client(2); c = lab.client(3)
    assert rules(b, [(1, 3)]) == [(1, 2, "", None)]
    assert rules(c, [(1, 3)]) == [(1, 2, "", None)]
    status(a, 5, "Zajęty — wrócę", 123456); barrier(a)
    change(b, (1, 5, "Zajęty — wrócę", 123456)); change(c, (1, 5, "Zajęty — wrócę", 123456))
    status(a, 0x16, "Nie ujawniaj", 987); barrier(a)
    change(b, (1, 1, "", None)); change(c, (1, 1, "", None))
    # Invisibility does not prevent messages; no presence leak is bundled with the message.
    b.send(0xb, words(1, 1, 8) + b"test\0"); assert take(b, 5) == words(3, 1, 1); assert take(a, 10)[16:] == b"test\0"
    rules(a, [(2, 3)])
    status(a, 0x8004, "Znajomi", 456); barrier(a)
    change(b, (1, 4, "Znajomi", 456)); assert rules(c, [(1, 3)]) == [(1, 1, "", None)]
    a.send(0xd, words(2) + b"\x04"); barrier(a); change(b, (1, 1, "", None))
    a.send(0xe, words(2) + b"\x04"); barrier(a); change(b, (1, 4, "Znajomi", 456))
    a.send(0xe, words(2) + b"\x02"); barrier(a); change(b, (1, 1, "", None))
    status(a, 4, "Publiczny"); barrier(a)
    change(c, (1, 4, "Publiczny", None))  # buddy-only remains permanently hidden
    assert rules(b, [(1, 3)]) == [(1, 1, "", None)]
    a.send(0xe, words(2) + b"\x01"); barrier(a); change(b, (1, 4, "Publiczny", None))
    b.send(0xe, words(1) + b"\x01"); barrier(b); b.mail = []
    status(a, 3); barrier(a); b.quiet(.65)  # unsubscribed
    assert rules(b, [(1, 3)]) == [(1, 3, "", None)]
    a.close(); change(b, (1, 1, "", None))

def offline_and_initial(lab):
    lab.start(); a = lab.client(1); b = lab.client(2); c = lab.client(3)
    rules(a, [(2, 3), (3, 4)]); rules(b, [(1, 3)]); rules(c, [(1, 3)])
    status(a, 0x8015, "Wrócę później", 12345); a.closed()
    change(b, (1, 0x15, "Wrócę później", 12345))
    assert rules(c, [(1, 3)]) == [(1, 1, "", None)]
    lab.stop(hard=True); lab.start(); b = lab.client(2); c = lab.client(3)
    assert rules(b, [(1, 3)]) == [(1, 0x15, "Wrócę później", 12345)]
    assert rules(c, [(1, 3)]) == [(1, 1, "", None)]
    a = lab.client(1, ready=False)
    assert rules(b, [(1, 3)]) == [(1, 1, "", None)]
    status(a, 0x8004, "Przed listą", 42)
    assert rules(c, [(1, 3)]) == [(1, 1, "", None)]
    rules(a, [(2, 3), (3, 4)])
    change(b, (1, 4, "Przed listą", 42))
    assert rules(c, [(1, 3)]) == [(1, 1, "", None)]
    status(a, 0x15, "ą" * 70, 55); a.closed(); change(b, (1, 0x15, "ą" * 70, 55))
    a = lab.client(1); barrier(a); status(a, 0x15, ""); a.closed()
    assert rules(b, [(1, 3)]) == [(1, 1, "", None)]
    a = lab.client(1); barrier(a); status(a, 4, "x" * 71); a.closed()
    assert rules(b, [(1, 3)]) == [(1, 1, "", None)]
    a = lab.client()
    body = words(1, oracle(PASSWORD, a.seed), 0x8015, 0x20) + bytes(13) + bytes([255, 0xbe]) + "Na starcie".encode("cp1250") + b"\0" + words(99)
    a.send(0x15, body); assert a.receive() == (3, b"")
    assert rules(b, [(1, 3)]) == [(1, 1, "", None)]
    a.send(0x10, words(2) + b"\x03" + words(3) + b"\x04"); a.closed()
    change(b, (1, 0x15, "Na starcie", 99)); assert rules(c, [(1, 3)]) == [(1, 1, "", None)]

def get_archive(c):
    c.send(0x16, b"\x02"); result = b""
    while True:
        b = take(c, 0x10); assert b[0] in (4, 6); result += b[1:]
        if b[0] == 6: return result

def put_archive(c, data):
    chunks = [data[i:i+2047] for i in range(0, len(data), 2047)] or [b""]
    if data and len(data) % 2047 == 0: chunks.append(b"")  # explicit completion extension
    for i, chunk in enumerate(chunks): c.send(0x16, bytes([int(i != 0)]) + chunk)
    for i in range(len(chunks)): assert take(c, 0x10) == bytes([0 if i == 0 else 2])

def archives(lab):
    lab.start(extra="archive_seconds=1\n"); a = lab.client(1); b = lab.client(2)
    assert get_archive(a) == b""
    for n in [0, 1, 2046, 2047, 2048, 4094, 4095]:
        data = ("ą" * n).encode("cp1250"); put_archive(a, data)
        assert get_archive(a) == data; assert get_archive(b) == b""
    assert lab.sql("SELECT version FROM contact_archives WHERE owner=1") == [(7,)]
    previous = get_archive(a)
    a.send(0x16, b"\0" + b"a" * 2047); a.quiet(.2); assert get_archive(a) == previous
    a.send(0x16, b"\1" + b"b" * 2047); a.quiet(.2); assert get_archive(a) == previous
    a.close(); until(lambda: lab.sql("SELECT count(*) FROM contact_staging") == [(0,)])
    a = lab.client(1); assert get_archive(a) == previous
    a.send(0x16, b"\0" + b"a" * 2047); assert get_archive(a) == previous
    a.closed()  # no false success on timeout or exact-full last chunk
    a = lab.client(1); assert get_archive(a) == previous
    a.send(0x16, b"\0" + b"a" * 2047); put_archive(a, b"Nowa;lista;123\r\n")
    assert get_archive(a) == b"Nowa;lista;123\r\n"
    a.send(0x16, b"\0" + b"partial" * 292 + b"abc")  # 2047-byte stage
    lab.stop(hard=True); lab.start(); a = lab.client(1)
    assert get_archive(a) == b"Nowa;lista;123\r\n"
    assert lab.sql("SELECT count(*) FROM contact_staging") == [(0,)]
    a.send(0x16, b"\1orphan"); a.closed()
    a = lab.client(1); assert get_archive(a) == b"Nowa;lista;123\r\n"
    a.send(0x16, b"\0bad\0data"); a.closed()
    a = lab.client(1); assert get_archive(a) == b"Nowa;lista;123\r\n"
    assert lab.sql("PRAGMA integrity_check") == [("ok",)]

def subscriptions(lab):
    lab.start(); a = lab.client(1)
    contacts = [(n, 3) for n in range(1000, 5000)]
    for offset in range(0, 4000, 400): a.send(0xf, b"".join(words(n) + bytes([flags]) for n, flags in contacts[offset:offset+400]))
    a.send(0x10); seen = []
    while len(seen) < 4000: seen.extend(parse_status(take(a, 0x11), True))
    assert len(set(row[0] for row in seen)) == 4000 and all(row[1:] == (1, "", None) for row in seen)
    barrier(a)
    b = lab.client(2); status(b, 4, "Nie subskrybowano"); barrier(b); a.quiet(.65)
    a.send(0xd, words(2) + b"\3"); a.closed()  # cannot exceed the contact quota
    assert lab.sql("SELECT count(*) FROM contact_rules WHERE owner=1") == [(4000,)]

def archive_crash(lab, stage):
    lab.start(fault=stage);a=lab.client(1);put_archive(a,b"previous")
    a.send(0x16,b"\0rGG_archive_fault")
    lab.server.process.wait(timeout=5);assert lab.server.process.returncode==86
    lab.stop();lab.start();a=lab.client(1)
    assert get_archive(a)==(b"previous" if stage=="before_archive_commit" else b"rGG_archive_fault")
    assert lab.sql("PRAGMA integrity_check")==[("ok",)]

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--server",type=Path,required=True);parser.add_argument("--admin",type=Path,required=True);parser.add_argument("--faults",action="store_true");args=parser.parse_args()
    cases=[("presence changes, invisibility and viewer-specific privacy",visibility),("initial privacy and durable offline descriptions",offline_and_initial),("contact archive boundaries, staging, replacement and restart",archives),("subscription paging and limits",subscriptions)]
    if args.faults:
        for stage in ["before_archive_commit","after_archive_commit"]:cases.append(("crash "+stage,lambda lab,stage=stage:archive_crash(lab,stage)))
    for name,case in cases:
        with tempfile.TemporaryDirectory(prefix="rgg-presence-") as root:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
            try: case(lab);print("PASS:",name,flush=True)
            finally: lab.stop()
    print("PASS: synthetic GG6 presence/contact suite; native-client qualification remains open",flush=True)

if __name__=="__main__":main()
