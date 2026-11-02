# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Offline recovery: mandatory snapshot, retained data, migration and atomic crash boundaries."""
import argparse
import contextlib
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path
from backups_cli import prepare, create, verify, restore, hashes
from gg6_sessions import Lab, PASSWORD
from gg6_presence_contacts import barrier, get_archive
from gg6_directory import query
from gg6_images import text, download
from password_cli import NEW, login, change


def sql(path, statement, values=()):
    with contextlib.closing(sqlite3.connect(str(path))) as db:
        with db:
            return db.execute(statement, values).fetchall()


def state(lab):
    return {name: sql(lab.data / 'rgg.sqlite', 'SELECT * FROM ' + name + ' ORDER BY 1')
            for name in ['accounts', 'secret_nonces', 'admin_audit', 'password_change_budget']}


def reset(lab, *, backup=None, new=NEW, uin=1, confirm=None, raw=None, ok=True,
          fault=None, backup_fault=None, omit_backup=False):
    if backup is None:
        backup = lab.root / 'reset-backup'
    env = os.environ.copy()
    for name, value in [('RGG_TEST_PASSWORD_RESET_CRASH', fault), ('RGG_TEST_BACKUP_CRASH', backup_fault)]:
        env.pop(name, None)
        if value:
            env[name] = value
    if raw is None:
        raw = json.dumps({'confirm_uin': uin if confirm is None else confirm,
                          'new_password': new.decode()}).encode()
    args = [str(lab.admin), 'password-reset', '--data-dir', str(lab.data), '--uin', str(uin)]
    if not omit_backup:
        args += ['--backup', str(backup)]
    result = subprocess.run(args, input=raw, capture_output=True, timeout=30, env=env)
    for secret in [PASSWORD, NEW, new]:
        if len(secret) >= 8:
            assert secret not in result.stdout + result.stderr
    if ok:
        assert result.returncode == 0, result.stderr
        return json.loads(result.stdout)
    assert result.returncode != 0
    return result


def normal(lab):
    prepare(lab)
    database = lab.data / 'rgg.sqlite'
    sql(database, "INSERT INTO password_change_budget VALUES(1,CAST(strftime('%s','now') AS INTEGER),5)")
    sql(database, "INSERT INTO password_change_budget VALUES(2,CAST(strftime('%s','now') AS INTEGER),3)")
    before = state(lab)
    preserved = {name: sql(database, 'SELECT * FROM ' + name + ' ORDER BY 1')
                 for name in ['messages', 'deliveries', 'image_offers', 'contact_rules', 'invitations', 'retired_uins']}
    key = (lab.data / 'keys.toml').read_bytes()
    generation = sql(database, 'SELECT session_generation FROM accounts WHERE uin=1')[0][0]
    backup = lab.root / 'before-reset'
    result = reset(lab, backup=backup)
    assert result == {'password_reset': True, 'uin': 1, 'credential_version': 2, 'backup_created': True}
    assert verify(lab, backup)['verified']
    saved = hashes(backup)
    for name, rows in before.items():
        assert sql(backup / 'rgg.sqlite', 'SELECT * FROM ' + name + ' ORDER BY 1') == rows
    for name, rows in preserved.items():
        assert sql(database, 'SELECT * FROM ' + name + ' ORDER BY 1') == rows, name
    assert (lab.data / 'keys.toml').read_bytes() == key
    assert sql(database, 'SELECT session_generation FROM accounts WHERE uin=1') == [(generation + 1,)]
    assert sql(database, 'SELECT uin,attempts FROM password_change_budget') == [(2, 3)]
    events = lab.run('password-audit')['events']
    assert len(events) == 1 and events[0]['action'] == 'password_reset' and events[0]['actor'] == 'local_cli_recovery'
    assert events[0]['previous_version'] == 1 and events[0]['credential_version'] == 2 and events[0]['uin'] == 1
    assert len(state(lab)['secret_nonces']) == len(before['secret_nonces']) + 1
    lab.start()
    login(lab, 1, PASSWORD, ok=False)
    b = lab.client(2); text(b)
    a = login(lab, 1, NEW); barrier(a)
    assert query(a, 2)['firstname'] == 'Próba kopii'
    assert get_archive(a) == 'Znajomy;2\r\n'.encode('cp1250')
    download(a, b)
    blocked = lab.root / 'live-backup'
    assert b'another process owns' in reset(lab, backup=blocked, new=PASSWORD, ok=False).stderr
    assert not blocked.exists()
    lab.stop()
    after = lab.root / 'after-reset'; create(lab, after)
    root, source = lab.root, lab.data
    for name, snapshot, good, bad, count in [('old', backup, PASSWORD, NEW, 0), ('new', after, NEW, PASSWORD, 1)]:
        destination = root / (name + '-data'); restore(lab, snapshot, destination)
        runtime = root / (name + '-runtime'); runtime.mkdir(); (runtime / 'data').symlink_to(destination, target_is_directory=True)
        lab.root, lab.data = runtime, destination
        assert len(lab.run('password-audit')['events']) == count
        lab.start(); login(lab, 1, bad, ok=False); barrier(login(lab, 1, good)); lab.stop()
    lab.root, lab.data = root, source
    assert hashes(backup) == saved
    for file in source.glob('rgg.sqlite*'):
        assert PASSWORD not in file.read_bytes() and NEW not in file.read_bytes()
    lab.run('retire', '--uin', '1')
    assert lab.run('password-audit')['events'][0]['action'] == 'password_reset'
    assert sql(database, 'PRAGMA integrity_check') == [('ok',)] and not sql(database, 'PRAGMA foreign_key_check')


def refusal(lab):
    before = state(lab)
    backup = lab.root / 'reset-backup'
    for new in [b'short', b'1234567890123456', 'Zolcą12345'.encode(), b'has a space']:
        reset(lab, new=new, ok=False)
        assert not backup.exists()
    for raw in [b'{}', b'{bad', b'x' * 4097, b'{"confirm_uin":1,"new_password":"NewLocalTest8!","unknown":1}',
                b'{"confirm_uin":1,"confirm_uin":1,"new_password":"NewLocalTest8!"}']:
        reset(lab, raw=raw, ok=False); assert not backup.exists()
    reset(lab, confirm=2, ok=False); reset(lab, omit_backup=True, ok=False)
    assert not backup.exists() and state(lab) == before
    # A complete backup may remain after a valid request is refused; it is never deleted.
    for n, values in enumerate([{'new': PASSWORD}, {'uin': 555}]):
        snapshot = lab.root / ('refused-' + str(n))
        reset(lab, backup=snapshot, ok=False, **values)
        assert verify(lab, snapshot)['verified'] and state(lab) == before
    lab.run('retire', '--uin', '3'); retired_state = state(lab)
    reset(lab, uin=3, ok=False); assert state(lab) == retired_state
    saved = hashes(backup)
    reset(lab, ok=False); assert hashes(backup) == saved and state(lab) == retired_state
    pending = lab.data / 'keys.pending'; pending.write_bytes((lab.data / 'keys.toml').read_bytes()); pending.chmod(0o600)
    assert b'pending key rotation' in reset(lab, backup=lab.root / 'pending-backup', ok=False).stderr
    assert not (lab.root / 'pending-backup').exists(); pending.unlink()
    key = lab.data / 'keys.toml'; held = lab.data / 'keys.saved'; key.rename(held)
    reset(lab, ok=False); assert not key.exists(); held.rename(key)
    data = lab.data; lab.data = lab.root / 'uninitialized'
    reset(lab, ok=False); assert not lab.data.exists(); lab.data = data
    helper = Path(__file__).resolve().parents[1] / 'tools/reset_password.py'
    result = subprocess.run([sys.executable, str(helper), '--admin', str(lab.admin), '--data-dir', str(data),
                             '--uin', '1', '--backup', str(lab.root / 'helper-backup')],
                            input=b'', capture_output=True, timeout=10)
    assert result.returncode != 0 and not (lab.root / 'helper-backup').exists()
    assert state(lab) == retired_state


def crash(lab, stage):
    database = lab.data / 'rgg.sqlite'
    sql(database, "INSERT INTO password_change_budget VALUES(1,CAST(strftime('%s','now') AS INTEGER),5)")
    before = state(lab)
    generation = sql(database, 'SELECT session_generation FROM accounts WHERE uin=1')[0][0]
    backup = lab.root / 'reset-backup'
    backup_stage = stage == 'before_backup_commit'
    result = reset(lab, ok=False, fault=None if backup_stage else stage,
                   backup_fault=stage if backup_stage else None)
    assert result.returncode == 86, result.stderr
    committed = stage == 'after_reset_commit'
    if backup_stage:
        assert not (backup / 'manifest.json').exists() and (backup / 'manifest.pending').exists()
        verify(lab, backup, ok=False)
    else:
        assert verify(lab, backup)['verified']
        for name, rows in before.items():
            assert sql(backup / 'rgg.sqlite', 'SELECT * FROM ' + name + ' ORDER BY 1') == rows
    if not committed:
        assert state(lab) == before
    assert sql(database, 'SELECT credential_version,session_generation FROM accounts WHERE uin=1') == [(1 + committed, generation + committed)]
    assert len(state(lab)['secret_nonces']) == len(before['secret_nonces']) + committed
    assert len(lab.run('password-audit')['events']) == int(committed)
    assert sql(database, 'SELECT count(*) FROM password_change_budget') == [(int(not committed),)]
    lab.start(); login(lab, 1, PASSWORD if committed else NEW, ok=False)
    barrier(login(lab, 1, NEW if committed else PASSWORD)); lab.stop()
    assert sql(database, 'PRAGMA integrity_check') == [('ok',)] and not sql(database, 'PRAGMA foreign_key_check')


def migration(server, admin, root, baseline, empty):
    lab = Lab(baseline / 'rgg-server', baseline / 'rgg-admin', root)
    try:
        prepare(lab)
        change(lab); change(lab, old=NEW, new=PASSWORD)
        database = lab.data / 'rgg.sqlite'
        # Include the historical web action. This SQL fixture does not claim a web interaction.
        sql(database, "UPDATE admin_audit SET actor='local_web_with_current_password' WHERE id=2")
        if empty:
            sql(database, 'DELETE FROM admin_audit')
        rows = sql(database, 'SELECT * FROM admin_audit ORDER BY id')
        old_migrations = sql(database, 'SELECT version,checksum FROM _sqlx_migrations ORDER BY version')
        assert len(old_migrations) == 13
        assert sql(database, "SELECT seq FROM sqlite_sequence WHERE name='admin_audit'") == [(2,)]
        old = root / 'schema13'; create(lab, old); saved = hashes(old)
        lab.binary, lab.admin = server, admin
        assert reset(lab)['credential_version'] == 4
        assert sql(database, 'SELECT max(version) FROM _sqlx_migrations') == [(14,)]
        assert sql(database, 'SELECT version,checksum FROM _sqlx_migrations WHERE version<=13 ORDER BY version') == old_migrations
        assert sql(database, 'SELECT * FROM admin_audit WHERE id<3 ORDER BY id') == rows
        assert lab.run('password-audit')['events'][0]['id'] == 3
        lab.start(); b = lab.client(2); text(b); a = login(lab, 1, NEW); barrier(a); download(a, b); lab.stop()
        restored = root / 'restored13'
        lab.admin = baseline / 'rgg-admin'; restore(lab, old, restored)
        lab.data = restored; lab.admin = admin
        assert reset(lab, backup=root / 'restored-backup')['credential_version'] == 4
        assert lab.run('password-audit')['events'][0]['id'] == 3
        assert hashes(old) == saved
        assert sql(restored / 'rgg.sqlite', 'PRAGMA integrity_check') == [('ok',)]
    finally:
        lab.stop()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    parser.add_argument('--baseline-dir', type=Path)
    parser.add_argument('--faults', action='store_true')
    args = parser.parse_args()
    cases = [('mandatory verified snapshot, retained data, old/new login and restore', normal),
             ('invalid input, confirmation, active server, missing keys, pending rotation and backup refusal', refusal)]
    if args.faults:
        for stage in ['before_backup_commit', 'before_reset_commit', 'after_reset_commit']:
            cases.append((stage, lambda lab, stage=stage: crash(lab, stage)))
    for name, test in cases:
        with tempfile.TemporaryDirectory(prefix='rgg-password-reset-') as root:
            lab = Lab(args.server.resolve(), args.admin.resolve(), Path(root))
            try:
                test(lab); print('PASS:', name, flush=True)
            finally:
                lab.stop()
    if args.baseline_dir:
        for empty in [False, True]:
            with tempfile.TemporaryDirectory(prefix='rgg-reset-migration-') as root:
                migration(args.server.resolve(), args.admin.resolve(), Path(root), args.baseline_dir.resolve(), empty)
                print('PASS: real dev31 schema13→14, old backup restore, audit IDs (empty=' + str(empty) + ')', flush=True)


if __name__ == '__main__':
    main()
