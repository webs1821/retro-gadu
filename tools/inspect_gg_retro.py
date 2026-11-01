# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Read-only preflight of an OFFLINE gg-retro snapshot; never imports or decrypts credentials."""
import argparse
import hashlib
import json
import re
import sqlite3
import stat
import sys
import time
from pathlib import Path

REFERENCE = Path(__file__).resolve().parents[1] / 'docs/implementation/gg-retro-schema-reference.json'
MAX_BYTES = 2 * 1024**3

class Rejected(Exception):
    pass

def source_state(path):
    try:
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_size > MAX_BYTES:
            raise Rejected('source_must_be_a_bounded_regular_file')
        if sys.platform != 'win32' and info.st_mode & 0o077:
            raise Rejected('source_permissions_must_be_owner_only')
        if any(Path(str(path) + suffix).exists() or Path(str(path) + suffix).is_symlink()
               for suffix in ('-wal', '-shm', '-journal')):
            raise Rejected('standalone_offline_snapshot_required')
        return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
    except OSError as error:
        raise Rejected('source_unavailable') from error

def digest(path):
    result = hashlib.sha256()
    total = 0
    with path.open('rb') as stream:
        while True:
            chunk = stream.read(65536)
            if not chunk:
                return result.hexdigest()
            total += len(chunk)
            if total > MAX_BYTES:
                raise Rejected('source_exceeds_size_limit')
            result.update(chunk)

def scalar(db, query):
    value = db.execute(query).fetchone()[0]
    return 0 if value is None else value

def check_schema(db, reference):
    # Table names in queries below are fixed identifiers from the pinned reference, never SQL from the DB.
    objects = db.execute("SELECT type,name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'").fetchall()
    if any(kind in ('trigger', 'view') for kind, _ in objects):
        raise Rejected('unexpected_view_or_trigger')
    names = {name for kind, name in objects if kind == 'table'}
    if names != set(reference['tables']) | {'_sqlx_migrations'}:
        raise Rejected('unsupported_source_tables')
    for name, expected in reference['tables'].items():
        rows = db.execute('SELECT * FROM pragma_table_info(?) LIMIT ?', (name,len(expected)+1)).fetchall()
        actual = [dict(name=r[1], type=r[2], notnull=r[3], default=r[4], pk=r[5]) for r in rows]
        if actual != expected:
            raise Rejected('unsupported_source_columns')
    if scalar(db,"SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE typeof(version)!='integer' OR typeof(success)!='integer' OR typeof(checksum)!='blob' OR length(checksum)!=48)"):
        raise Rejected('invalid_migration_metadata')
    rows = db.execute('SELECT version,success,checksum FROM _sqlx_migrations ORDER BY version LIMIT 100').fetchall()
    expected = reference['migrations']
    if len(rows) != len(expected):
        raise Rejected('unsupported_migration_history')
    for row, migration in zip(rows, expected):
        if (row[0] != migration['version'] or row[1] != 1 or
            not isinstance(row[2], bytes) or row[2].hex() != migration['sha384']):
            raise Rejected('migration_checksum_mismatch')

def summarize(db, reference):
    counts = {}
    for name in reference['tables']:
        if not re.fullmatch('[a-z_]+', name):
            raise Rejected('invalid_pinned_reference')
        counts[name] = scalar(db, 'SELECT count(*) FROM "' + name + '"')
    encrypted = scalar(db, "SELECT count(*) FROM users WHERE typeof(password)='text' AND substr(password,1,5)='ggv1:'")
    allocator = db.execute('SELECT id,next_uin FROM registration_state').fetchall()
    if len(allocator) != 1 or allocator[0][0] != 1 or not 112233 <= allocator[0][1] <= 16777216:
        raise Rejected('invalid_number_allocator')
    checks = {
        'unsupported_account_numbers': scalar(db, 'SELECT count(*) FROM users WHERE uin NOT BETWEEN 1 AND 16777215'),
        'unrecognized_secret_format': counts['users'] - encrypted,
        'pending_orphan_messages': scalar(db, 'SELECT count(*) FROM messages m WHERE delivered_at IS NULL AND (NOT EXISTS(SELECT 1 FROM users u WHERE u.uin=m.sender_uin) OR NOT EXISTS(SELECT 1 FROM users u WHERE u.uin=m.recipient_uin))'),
        'orphan_image_offers': scalar(db, 'SELECT count(*) FROM message_images i WHERE NOT EXISTS(SELECT 1 FROM messages m WHERE m.id=i.message_id)'),
        'orphan_contact_owners': scalar(db, 'SELECT count(*) FROM contact_lists c WHERE NOT EXISTS(SELECT 1 FROM users u WHERE u.uin=c.uin)'),
        'orphan_profile_owners': scalar(db, 'SELECT count(*) FROM public_profiles p WHERE NOT EXISTS(SELECT 1 FROM users u WHERE u.uin=p.uin)'),
        'orphan_presence_owners': scalar(db, 'SELECT count(*) FROM offline_presence p WHERE NOT EXISTS(SELECT 1 FROM users u WHERE u.uin=p.uin)'),
        'invalid_invite_counters': scalar(db, "SELECT count(*) FROM registration_invites WHERE typeof(code_hash)!='blob' OR length(code_hash)!=32 OR max_uses!=3 OR used NOT BETWEEN 0 AND 3 OR active NOT IN (0,1)"),
        'unsupported_profile_years': scalar(db, 'SELECT count(*) FROM public_profiles WHERE birthyear!=0 AND birthyear NOT BETWEEN 1900 AND 2106'),
    }
    messages = {
        'pending': scalar(db, 'SELECT count(*) FROM messages WHERE delivered_at IS NULL'),
        'transport_written_without_client_ack': scalar(db, 'SELECT count(*) FROM messages WHERE delivered_at IS NOT NULL'),
        'pending_with_formatting': scalar(db, 'SELECT count(*) FROM messages WHERE delivered_at IS NULL AND formatting IS NOT NULL'),
        'pending_conferences': scalar(db, 'SELECT count(*) FROM messages WHERE delivered_at IS NULL AND conference IS NOT NULL'),
        'image_offers': counts['message_images'],
    }
    invites = {
        'total': counts['registration_invites'],
        'used': scalar(db, 'SELECT sum(used) FROM registration_invites'),
        'active_remaining_uses': scalar(db, 'SELECT sum(max_uses-used) FROM registration_invites WHERE active=1'),
        'compatibility': 'legacy_case_sensitive_sha256_adapter_required',
    }
    return {
        'counts': counts, 'number_allocator_next_uin': allocator[0][1],
        'credentials': {'ggv1_prefix': encrypted, 'decryption': 'not_performed', 'password_policy': 'not_checked'},
        'messages': messages, 'invitations': invites, 'findings': checks,
        'known_missing_source_data': ['retired_uin_history', 'privacy_not_saved_in_offline_presence'],
        'required_before_import': ['verify_every_secret_with_legacy_key', 'preserve_legacy_invite_hashes_and_usage',
            'decode_and_validate_all_payloads', 'map_profiles_contacts_offline_privacy',
            'supply_reserved_uins_or_explicitly_accept_missing_history', 'implement_transactional_target_import'],
    }

def inspect(path, reference=REFERENCE):
    # Do not resolve() before lstat: a symlink passed as the source must be rejected.
    path = Path(path).absolute()
    initial = source_state(path)
    before = digest(path)
    spec = json.loads(Path(reference).read_text(encoding='utf-8'))
    deadline = time.monotonic() + 30
    db = sqlite3.connect(path.as_uri() + '?mode=ro&immutable=1', uri=True)
    try:
        db.execute('PRAGMA query_only=ON')
        db.execute('PRAGMA trusted_schema=OFF')
        if hasattr(db, 'enable_load_extension'):
            db.enable_load_extension(False)  # absent when extension loading was disabled at build time
        if hasattr(db, 'setlimit'):
            db.setlimit(sqlite3.SQLITE_LIMIT_LENGTH, 16 * 1024**2)
        db.set_progress_handler(lambda: int(time.monotonic() > deadline), 10000)
        check_schema(db, spec)
        if db.execute('PRAGMA integrity_check').fetchone() != ('ok',):
            raise Rejected('source_integrity_failed')
        if db.execute('PRAGMA foreign_key_check').fetchone() is not None:
            raise Rejected('source_foreign_keys_failed')
        result = summarize(db, spec)
    finally:
        db.close()
    if initial != source_state(path) or digest(path) != before or initial != source_state(path):
        raise Rejected('source_changed_during_inspection')
    return dict(format=1, inspected=True, migration_ready=False, source_sha256=before,
                source_reference_commit=spec['commit'], source_version=spec['version'], **result)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = inspect(args.source)
    except (Rejected, OSError, sqlite3.Error, ValueError, KeyError, TypeError) as error:
        # SQLite errors can contain data/SQL: output only our fixed error codes.
        code = str(error) if isinstance(error, Rejected) else 'source_inspection_failed'
        print(json.dumps(dict(inspected=False, migration_ready=False, error=code)))
        return 2
    print(json.dumps(result, indent=2))
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
