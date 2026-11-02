# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Complete local migration from synthetic gg-retro schema/data and independent libsodium vectors."""
import argparse,base64,copy,hashlib,json,os,sqlite3,struct,subprocess,sys,tempfile,zlib
from pathlib import Path
from gg6_sessions import Lab,PASSWORD,words
from gg6_presence_contacts import barrier,get_archive
from gg6_directory import query
from gg6_images import DATA,style,download
from legacy_inspection import fixture,database,ROOT
from backups_cli import create,restore
sys.path.insert(0,str(ROOT/'tools'))
from migrate_gg_retro import build_package

CODE='old-LEGACY_invite_1234567890'
def source(root):
    path=root/'source.sqlite';fixture(path)
    vectors=json.loads((ROOT/'tests/fixtures/gg-retro-vault-vectors.json').read_text())
    key=root/'legacy.key';key.write_bytes(bytes.fromhex(vectors['synthetic_key_hex']));key.chmod(0o600)
    with database(path) as db:
        db.execute("INSERT INTO users(uin,name,email,password,created_at) VALUES(3,'PRIVATE_NAME_3','private3@example.invalid','temporary',NULL)")
        for item in vectors['vectors']:
            db.execute('UPDATE users SET password=? WHERE uin=?',(item['encoded'],item['uin']))
        db.execute('UPDATE registration_invites SET code_hash=? WHERE code_hash=?',(hashlib.sha256(CODE.encode()).digest(),bytes([7])*32))
        db.execute('UPDATE users SET registration_invite_hash=? WHERE uin=1',(hashlib.sha256(CODE.encode()).digest(),))
        db.execute('UPDATE messages SET message=?,formatting=NULL,conference=NULL WHERE id=1',('Wiadomość offline',))
        db.execute('INSERT INTO messages(id,recipient_uin,sender_uin,seq,time,class,message,formatting,conference) VALUES(3,2,1,77,1002,4,?,?,?)',('\xa0',style(),b'\x01'+words(1,3)))
        db.execute('DELETE FROM message_images');db.execute('INSERT INTO message_images VALUES(3,?,?)',(len(DATA),zlib.crc32(DATA)))
        db.execute('UPDATE contact_lists SET data=? WHERE uin=1',('Znajomy;2\r\n'.encode('cp1250'),))
        db.execute('UPDATE offline_presence SET status=32789,description=?,return_time=123,privacy=? WHERE uin=1',('Nie ma mnie',words(2)+b'\x02'+words(4)+b'\x04'))
        db.execute("UPDATE public_profiles SET firstname='Łucja',gender=2,birthyear=1980 WHERE uin=1")
    return path,key
def payload(path):return build_package(path,[4],True)
def command(admin,key,data,target=None,ok=True,fault=None):
    args=[str(admin),'import-check' if target is None else 'import-apply','--legacy-key',str(key)]
    if target is not None:args+=['--data-dir',str(target)]
    env=os.environ.copy();env.pop('RGG_TEST_IMPORT_CRASH',None)
    if fault:env['RGG_TEST_IMPORT_CRASH']=fault
    p=subprocess.run(args,input=data,capture_output=True,timeout=30,env=env)
    for secret in [PASSWORD,CODE.encode(),b'PRIVATE_NAME',b'PRIVATE_MESSAGE','Wiadomość offline'.encode(),b'ggv1:']:assert secret not in p.stdout+p.stderr
    if ok:assert p.returncode==0,p.stderr;return json.loads(p.stdout)
    assert p.returncode!=0;return p
def lab_for(server,admin,root):
    lab=Lab.__new__(Lab);lab.binary=server;lab.admin=admin;lab.root=root;lab.data=root/'data';lab.server=None;lab.clients=[];return lab
def normal(server,admin,root):
    path,key=source(root);before=hashlib.sha256(path.read_bytes()).hexdigest();data=payload(path)
    checked=command(admin,key,data);assert checked['checked'] and checked['summary']['accounts']==3
    assert checked['summary']['pending_messages']==2 and checked['summary']['image_offers']==1 and checked['summary']['missing_account_creation_times']==1
    assert not (root/'data').exists();done=command(admin,key,data,root/'data');assert done['imported']
    lab=lab_for(server,admin,root)
    try:
        assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(14,)]
        assert lab.sql('SELECT uin FROM retired_uins')==[(4,)] and lab.sql('SELECT next_uin FROM number_allocator')==[(112250,)]
        assert lab.sql('SELECT flags FROM contact_rules WHERE owner=1 ORDER BY peer')==[(2,),(4,)]
        assert lab.sql('SELECT friends_only FROM account_privacy WHERE uin=1')==[(1,)]
        assert lab.sql('SELECT description,return_at FROM account_presence WHERE uin=1')==[('Nie ma mnie',123)]
        assert lab.sql('SELECT source_created_at FROM imported_account_metadata WHERE uin=3')==[(None,)]
        assert lab.sql('SELECT body,styles FROM messages WHERE id=2')==[(None,None)]
        assert lab.sql('SELECT package_sha256 FROM import_receipt')==[(hashlib.sha256(data).hexdigest(),)]
        lab.start();b=lab.client(2)
        kind,body=b.receive();assert kind==10 and body[:12]==words(1,7,1000) and body[16:]==b'Wiadomo\x9c\xe6 offline\0',(kind,body)
        kind,body=b.receive();assert kind==10 and body[:12]==words(1,77,1002) and body[16:]==b'\xa0\0'+b'\x01'+words(1,3)+style(),(kind,body)
        b.quiet();a=lab.client(1);barrier(a)
        assert query(a,2)['firstname']=='Łucja' and query(a,2)['gender']=='2'
        rows,_=query(b,fields={'firstname':'lucja','gender':'1'});assert len(rows)==1 and rows[0]['fmnumber']=='1'
        assert get_archive(a)=='Znajomy;2\r\n'.encode('cp1250');download(a,b)
        assert query(lab.client(3),2)['firstname']=='' # private account names never become directory fields
        lab.stop();backup=root/'backup';create(lab,backup);restore(lab,backup,root/'restored')
        # Imported invitation consumption/retirement uses the normal production registration path.
        from legacy_invitations import account
        account(lab,CODE,4,ok=False);assert account(lab,CODE)==112250;account(lab,CODE,ok=False)
        assert lab.sql('SELECT used FROM legacy_invitations WHERE active=1')==[(3,)]
        assert hashlib.sha256(path.read_bytes()).hexdigest()==before
        existing=(lab.data/'keys.toml').read_bytes();command(admin,key,data,lab.data,ok=False);assert (lab.data/'keys.toml').read_bytes()==existing
    finally:lab.stop()
def refused(server,admin,root):
    path,key=source(root);data=payload(path);base=json.loads(data)
    cases={
        'wrong-account-cipher':lambda p:p['users'][1].update(password=p['users'][0]['password']),
        'plaintext':lambda p:p['users'][0].update(password=PASSWORD.decode()),
        'missing-history':lambda p:p.update(missing_history_acknowledged=False),
        'duplicate-account':lambda p:p['users'].append(p['users'][0]),
        'reserved-collision':lambda p:p.update(reserved_uins=[1]),
        'orphan-pending':lambda p:p['messages'][0].update(recipient_uin=555),
        'bad-encoding':lambda p:p['messages'][0].update(message='emoji 🦊'),
        'bad-formatting':lambda p:p['messages'][0].update(formatting=base64.b64encode(b'bad').decode()),
        'wrong-image':lambda p:p['images'][0].update(crc32=42),
        'blocked-content':lambda p:p['presence'][0].update(privacy=base64.b64encode(words(2)+b'\x04').decode()),
        'unknown-field':lambda p:p.update(unknown='must reject'),
        'bad-counter':lambda p:p['invitations'][0].update(used=4),
        'queue-capacity':lambda p:p['messages'].extend([{**p['messages'][0],'id':n} for n in range(100,1100)]),
    }
    for name,change in cases.items():
        altered=copy.deepcopy(base);change(altered);target=root/('rejected-'+name)
        command(admin,key,json.dumps(altered).encode(),target,ok=False);assert not target.exists(),name
    wrong=root/'wrong.key';wrong.write_bytes(bytes([99])*32);wrong.chmod(0o600)
    command(admin,wrong,data,root/'wrong-key-target',ok=False);assert not (root/'wrong-key-target').exists()
    for name,value in [('short-key',bytes(31)),('long-key',bytes(33))]:
        invalid=root/name;invalid.write_bytes(value);invalid.chmod(0o600);target=root/(name+'-target')
        command(admin,invalid,data,target,ok=False);assert not target.exists()
    if os.name!='nt':
        linked=root/'linked-key';linked.symlink_to(key);command(admin,linked,data,root/'linked-target',ok=False);assert not (root/'linked-target').exists()
        key.chmod(0o644);command(admin,key,data,root/'public-key-target',ok=False);assert not (root/'public-key-target').exists();key.chmod(0o600)
    # The command-line wrapper also forwards only aggregate reports.
    reserved=root/'reserved.json';reserved.write_text('[4]')
    p=subprocess.run([sys.executable,str(ROOT/'tools/migrate_gg_retro.py'),'--source',str(path),'--admin',str(admin),'--legacy-key',str(key),'--reserved-uins',str(reserved),'--acknowledge-missing-history','--check'],capture_output=True,timeout=30)
    assert p.returncode==0,p.stderr;assert json.loads(p.stdout)['checked'] and PASSWORD not in p.stdout+p.stderr
def crash(server,admin,root,stage):
    path,key=source(root);data=payload(path);target=root/'data'
    p=command(admin,key,data,target,ok=False,fault=stage);assert p.returncode==86
    assert (target/'import-incomplete').exists()
    if stage=='before_import_database':assert not (target/'rgg.sqlite').exists()
    else:
        with database(target/'rgg.sqlite') as db:
            assert db.execute('SELECT count(*) FROM accounts').fetchone()[0]==(0 if stage=='before_import_commit' else 3)
            assert db.execute('SELECT count(*) FROM import_receipt').fetchone()[0]==(0 if stage=='before_import_commit' else 1)
    p=subprocess.run([str(admin),'stats','--data-dir',str(target)],capture_output=True);assert p.returncode!=0 and b'incomplete import' in p.stderr
    config=root/'blocked.toml';config.write_text("data_dir='data'\nhttp_bind='127.0.0.1:0'\n")
    p=subprocess.run([str(server),'--config',str(config)],capture_output=True,timeout=5);assert p.returncode!=0 and b'incomplete import' in p.stderr
    command(admin,key,data,target,ok=False);assert command(admin,key,data,root/'fresh')['imported']
def migration(server,admin,root,baseline):
    from backups_cli import prepare
    from legacy_invitations import seed
    from gg6_images import text
    lab=Lab(baseline/'rgg-server',baseline/'rgg-admin',root)
    try:
        seed(lab,CODE,used=2);prepare(lab);assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(9,)]
        backup=root/'schema9';create(lab,backup)
        lab.binary=server;lab.admin=admin;lab.run('stats')
        assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(14,)]
        assert lab.sql('SELECT used,active FROM legacy_invitations')==[(2,1)] and lab.sql('SELECT count(*) FROM import_receipt')==[(0,)]
        restored=root/'restored9';p=subprocess.run([str(baseline/'rgg-admin'),'restore','--backup',str(backup),'--data-dir',str(restored)],capture_output=True,timeout=15);assert p.returncode==0,p.stderr
        p=subprocess.run([str(admin),'stats','--data-dir',str(restored)],capture_output=True,timeout=15);assert p.returncode==0 and json.loads(p.stdout)['registered']==2
        lab.start();b=lab.client(2);text(b);a=lab.client(1);barrier(a);download(a,b)
    finally:lab.stop()
def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);p.add_argument('--faults',action='store_true');p.add_argument('--baseline-dir',type=Path);a=p.parse_args()
    cases=[('full import, GG login/offline messages/images/profile/contacts/invites, source unchanged',normal),('invalid key/data/relations/limits rejected before creating target; wrapper check',refused)]
    if a.faults:
        for stage in ['before_import_database','before_import_commit','after_import_commit']:cases.append((stage,lambda s,a,r,stage=stage:crash(s,a,r,stage)))
    for name,test in cases:
        with tempfile.TemporaryDirectory(prefix='rgg-import-') as root:
            test(a.server.resolve(),a.admin.resolve(),Path(root));print('PASS:',name,flush=True)
    if a.baseline_dir:
        with tempfile.TemporaryDirectory(prefix='rgg-import-migration-') as root:
            migration(a.server.resolve(),a.admin.resolve(),Path(root),a.baseline_dir.resolve());print('PASS: real dev.14 schema-9 migration/restore; invites and queued images preserved',flush=True)
if __name__=='__main__':main()
