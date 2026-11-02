# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Unix private-directory guards, special/link refusal and unchanged live data."""
import argparse
import hashlib
import os
import subprocess
import tempfile
from pathlib import Path
from gg6_sessions import Lab


def call(admin, data, ok=False):
    result = subprocess.run([str(admin), 'init', '--data-dir', str(data)],
                            capture_output=True, timeout=4)
    assert (result.returncode == 0) == ok, result.stderr
    return result


def public_directories(admin, root):
    for mode in [0o755, 0o750, 0o770, 0o777]:
        data = root / ('mode-' + oct(mode)); data.mkdir(); data.chmod(mode)
        assert b'data directory must restrict access' in call(admin, data).stderr
        assert list(data.iterdir()) == [] and data.stat().st_mode & 0o777 == mode
        # Explicit operator correction; the program itself must not chmod existing data.
        data.chmod(0o700); call(admin, data, ok=True)
        assert (data / 'keys.toml').stat().st_mode & 0o777 == 0o600
    missing = root / 'new' / 'data'; call(admin, missing, ok=True)
    assert missing.stat().st_mode & 0o777 == 0o700
    assert missing.parent.stat().st_mode & 0o777 == 0o700


def unsafe_files(admin, root):
    outside = root / 'outside-canary'; outside.write_bytes(b'KEEP SYNTHETIC DATA')
    original = outside.read_bytes()
    names = ['rgg.lock', 'rgg.sqlite', 'rgg.sqlite-wal', 'rgg.sqlite-shm', 'rgg.sqlite-journal']
    for name in names:
        for kind in ['symlink', 'dangling', 'hardlink', 'fifo', 'directory']:
            data = root / (name + '-' + kind); data.mkdir(mode=0o700)
            target = data / name
            if kind == 'symlink': target.symlink_to(outside)
            elif kind == 'dangling': target.symlink_to(root / 'not-created')
            elif kind == 'hardlink': os.link(outside, target)
            elif kind == 'fifo': os.mkfifo(target, mode=0o600)
            else: target.mkdir(mode=0o700)
            assert b'unsafe lock or database file' in call(admin, data).stderr, (name, kind)
            assert {p.name for p in data.iterdir()} == {name}
            assert outside.read_bytes() == original and not (root / 'not-created').exists()
            # Unlink only our own synthetic entry; avoid retaining extra hard links between cases.
            if kind == 'directory': target.rmdir()
            else: target.unlink()
    data = root / 'lock-mode'; data.mkdir(mode=0o700)
    lock = data / 'rgg.lock'; lock.write_bytes(b''); lock.chmod(0o644)
    assert b'unsafe lock or database file' in call(admin, data).stderr
    assert lock.stat().st_mode & 0o777 == 0o644 and not (data / 'rgg.sqlite').exists()
    lock.chmod(0o600); call(admin, data, ok=True)


def existing_data(server, admin, root):
    lab = Lab(server, admin, root)
    try:
        data = lab.data
        frozen = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in data.iterdir() if p.is_file()}
        data.chmod(0o755)
        assert b'data directory must restrict access' in call(admin, data).stderr
        config = root / 'refused.toml'; config.write_text("data_dir='data'\nhttp_bind='127.0.0.1:0'\n")
        refused = subprocess.run([str(server), '--config', str(config)], capture_output=True, timeout=5)
        assert refused.returncode != 0 and b'data directory must restrict access' in refused.stderr
        assert {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in data.iterdir() if p.is_file()} == frozen
        data.chmod(0o700)
        alias = root / 'alias'; alias.symlink_to(data, target_is_directory=True)
        call(admin, alias, ok=True)
        lab.start(); a = lab.client(1); b = lab.client(2)
        assert a.message(2, 1, 'Private directory test') == 3
        kind, body = b.receive(); assert kind == 10 and body[16:] == b'Private directory test\0'
        assert b'another process owns' in call(admin, alias).stderr
        lab.stop()
        assert lab.sql('PRAGMA integrity_check') == [('ok',)] and lab.sql('PRAGMA foreign_key_check') == []
    finally:
        lab.data.chmod(0o700)
        lab.stop()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    args = parser.parse_args()
    assert os.name == 'posix', 'Unix mode qualification; Windows ACLs are separate'
    server, admin = args.server.resolve(), args.admin.resolve()
    for name, test in [('existing public directory refused, explicit repair and private creation', lambda r: public_directories(admin, r)),
                       ('25 link/special-file cases plus insecure lock refused before mutation', lambda r: unsafe_files(admin, r)),
                       ('existing data immutable on refusal, alias lock, GG login and messages', lambda r: existing_data(server, admin, r))]:
        with tempfile.TemporaryDirectory(prefix='rgg-directory-') as temporary:
            test(Path(temporary)); print('PASS:', name, flush=True)


if __name__ == '__main__':
    main()
