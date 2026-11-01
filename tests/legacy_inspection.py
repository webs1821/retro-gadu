# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Synthetic databases generated from schema FACTS, not copied upstream migrations or live data."""
import hashlib,json,os,sqlite3,subprocess,sys,tempfile
from contextlib import contextmanager
from unittest.mock import patch
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'tools'))
from inspect_gg_retro import inspect,Rejected,REFERENCE
import inspect_gg_retro as preflight
PRIVATE=['PRIVATE_NAME_DO_NOT_PRINT','private-person@example.invalid','PRIVATE_MESSAGE_DO_NOT_PRINT','not-a-real-password']
@contextmanager
def database(path):
    db=sqlite3.connect(str(path))
    try:
        with db:yield db
    finally:db.close()
def fixture(path):
    spec=json.loads(REFERENCE.read_text())
    with database(path) as db:
        for table,columns in spec['tables'].items():
            definitions=[];primary=[]
            for col in columns:
                declaration='"'+col['name']+'" '+col['type']
                if col['notnull']:declaration+=' NOT NULL'
                if col['default'] is not None:declaration+=' DEFAULT '+col['default']
                definitions.append(declaration)
                if col['pk']:primary.append((col['pk'],col['name']))
            if primary:definitions.append('PRIMARY KEY('+','.join('"'+name+'"' for _,name in sorted(primary))+')')
            db.execute('CREATE TABLE "'+table+'"('+','.join(definitions)+')')
        db.execute('CREATE TABLE _sqlx_migrations(version INTEGER PRIMARY KEY,success INTEGER,checksum BLOB)')
        db.executemany('INSERT INTO _sqlx_migrations VALUES(?,1,?)',[(m['version'],bytes.fromhex(m['sha384'])) for m in spec['migrations']])
        for uin in [1,2]:db.execute('INSERT INTO users(uin,name,email,password) VALUES(?,?,?,?)',(uin,PRIVATE[0],PRIVATE[1]+str(uin),'ggv1:opaque-not-authenticated'))
        db.execute('INSERT INTO registration_state VALUES(1,112250)')
        db.execute('INSERT INTO registration_invites(code_hash,max_uses,used,active) VALUES(?,3,2,1)',(bytes([7])*32,))
        db.execute('INSERT INTO registration_invites(code_hash,max_uses,used,active) VALUES(?,3,1,0)',(bytes([8])*32,))
        db.execute('INSERT INTO messages(id,recipient_uin,sender_uin,seq,time,class,message,formatting,conference) VALUES(1,2,1,7,1000,4,?,?,?)',(PRIVATE[2],b'opaque-formatting',b'opaque-conference'))
        db.execute('INSERT INTO messages(id,recipient_uin,sender_uin,seq,time,class,message,delivered_at) VALUES(2,2,1,8,1001,4,?,CURRENT_TIMESTAMP)',(PRIVATE[2],))
        db.execute('INSERT INTO message_images VALUES(1,100,123)')
        db.execute('INSERT INTO contact_lists(uin,data) VALUES(1,?)',(b'PRIVATE_CONTACT_DATA',))
        db.execute("INSERT INTO public_profiles(uin,firstname,birthyear) VALUES(1,'PRIVATE_PROFILE',1980)")
        db.execute("INSERT INTO offline_presence(uin,status,description,privacy) VALUES(1,21,'PRIVATE_DESCRIPTION',?)",(bytes(5),))
    path.chmod(0o600)
def unchanged(path):
    before=hashlib.sha256(path.read_bytes()).hexdigest();stamp=path.stat().st_mtime_ns
    result=inspect(path);assert result['source_sha256']==before
    assert hashlib.sha256(path.read_bytes()).hexdigest()==before and path.stat().st_mtime_ns==stamp
    assert set(p.name for p in path.parent.iterdir())=={path.name}
    return result
def main():
    with tempfile.TemporaryDirectory(prefix='rgg-legacy-') as root:
        root=Path(root);path=root/'source.sqlite';fixture(path);r=unchanged(path)
        assert r['counts']['users']==2 and r['number_allocator_next_uin']==112250
        assert r['messages']['pending']==1 and r['messages']['transport_written_without_client_ack']==1
        assert r['invitations']['used']==3 and r['invitations']['active_remaining_uses']==1
        assert not r['migration_ready'] and r['credentials']['decryption']=='not_performed'
        result=subprocess.run([sys.executable,str(ROOT/'tools/inspect_gg_retro.py'),'--source',str(path)],capture_output=True,timeout=10)
        assert result.returncode==0
        for value in PRIVATE+['PRIVATE_CONTACT_DATA','PRIVATE_PROFILE','PRIVATE_DESCRIPTION','opaque-not-authenticated']:assert value.encode() not in result.stdout+result.stderr
        with database(path) as db:
            db.execute('UPDATE users SET password=? WHERE uin=2',(PRIVATE[3],));db.execute('UPDATE messages SET sender_uin=999 WHERE id=1')
        r=unchanged(path);assert r['findings']['unrecognized_secret_format']==1 and r['findings']['pending_orphan_messages']==1
        summarize=preflight.summarize
        def concurrent_change(db,spec):
            report=summarize(db,spec)
            with database(path) as writer:writer.execute("UPDATE users SET name='CHANGED_DURING_INSPECTION'")
            return report
        with patch('inspect_gg_retro.summarize',side_effect=concurrent_change):
            try:inspect(path);raise AssertionError('concurrently changed source accepted')
            except Rejected as error:assert str(error)=='source_changed_during_inspection'
        print('PASS: pinned schema, counts and semantic gaps; read-only files and redacted output')
    for case in ['version','checksum','columns','table','trigger','wal','journal','symlink','permissions','oversize']:
        with tempfile.TemporaryDirectory(prefix='rgg-legacy-reject-') as directory:
            root=Path(directory);path=root/'source.sqlite';fixture(path)
            if case in ['version','checksum','columns','table','trigger']:
                with database(path) as db:
                    db.execute({'version':'UPDATE _sqlx_migrations SET version=999 WHERE version=20261031010000','checksum':'UPDATE _sqlx_migrations SET checksum=zeroblob(48)','columns':'ALTER TABLE users ADD COLUMN unknown_field TEXT','table':'CREATE TABLE unknown_table(id INTEGER)','trigger':'CREATE TRIGGER unknown_trigger AFTER INSERT ON users BEGIN DELETE FROM users; END'}[case])
            elif case in ['wal','journal']:Path(str(path)+'-'+case).write_bytes(b'pending data')
            elif case=='symlink':
                if os.name=='nt':continue
                link=root/'link.sqlite';link.symlink_to(path);path=link
            elif case=='permissions':
                if os.name=='nt':continue
                path.chmod(0o644)
            else:
                with patch('inspect_gg_retro.MAX_BYTES',path.stat().st_size-1):
                    try:inspect(path);raise AssertionError('size limit ignored')
                    except Rejected:pass
                continue
            try:inspect(path);raise AssertionError('invalid source accepted: '+case)
            except Rejected:pass
    print('PASS: unsupported schema/history, sidecars, symlink, permissions and size refused')
if __name__=='__main__':main()
