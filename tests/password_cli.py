# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Authenticated offline password changes, real GG6 logins and atomic crash boundaries."""
import argparse,json,os,subprocess,sys,tempfile
from pathlib import Path
from gg6_sessions import Lab,PASSWORD
from gg6_presence_contacts import barrier,get_archive
from gg6_directory import query
from gg6_images import text,download
from backups_cli import prepare,create,restore,hashes

NEW=b'NewLocalTest8!'
def change(lab,old=PASSWORD,new=NEW,uin=1,ok=True,fault=None,raw=None):
    env=os.environ.copy();env.pop('RGG_TEST_PASSWORD_CRASH',None)
    if fault:env['RGG_TEST_PASSWORD_CRASH']=fault
    data=json.dumps({'current_password':old.decode(),'new_password':new.decode()}).encode() if raw is None else raw
    p=subprocess.run([str(lab.admin),'password-change','--data-dir',str(lab.data),'--uin',str(uin)],input=data,capture_output=True,timeout=15,env=env)
    for value in (PASSWORD,NEW,old,new):
        if len(value)>7:assert value not in p.stdout+p.stderr
    if ok:assert p.returncode==0,p.stderr;return json.loads(p.stdout)
    assert p.returncode!=0;return p
def login(lab,uin,password,ok=True):
    c=lab.client();assert c.login(uin,password)==((3,b'') if ok else (9,b''))
    if not ok:c.closed()
    return c
def normal(lab):
    prepare(lab);backup=lab.root/'before';create(lab,backup);saved=hashes(backup)
    original=lab.sql('SELECT email,created_at,active FROM accounts WHERE uin=1')
    key=(lab.data/'keys.toml').read_bytes();nonces=lab.sql('SELECT count(*) FROM secret_nonces')[0][0]
    result=change(lab);assert result=={'password_changed':True,'uin':1,'credential_version':2}
    assert lab.sql('SELECT email,created_at,active FROM accounts WHERE uin=1')==original
    assert (lab.data/'keys.toml').read_bytes()==key and lab.sql('SELECT count(*) FROM secret_nonces')==[(nonces+1,)]
    assert lab.sql('SELECT used FROM invitations')==[(3,)] and lab.sql('SELECT uin FROM retired_uins')==[(3,)]
    event=lab.run('password-audit')['events'];assert len(event)==1 and event[0]['uin']==1 and event[0]['previous_version']==1 and event[0]['credential_version']==2
    change(lab,ok=False);assert len(lab.run('password-audit')['events'])==1
    lab.start();login(lab,1,PASSWORD,ok=False);b=lab.client(2);text(b);a=login(lab,1,NEW);barrier(a)
    assert query(a,2)['firstname']=='Próba kopii';assert get_archive(a)=='Znajomy;2\r\n'.encode('cp1250');download(a,b)
    assert b'another process owns' in change(lab,old=NEW,new=PASSWORD,ok=False).stderr
    lab.stop();after=lab.root/'after';create(lab,after)
    root=lab.root;source=lab.data
    for name,snapshot,good,bad,count in [('old',backup,PASSWORD,NEW,0),('new',after,NEW,PASSWORD,1)]:
        destination=root/(name+'-data');restore(lab,snapshot,destination)
        runtime=root/(name+'-runtime');runtime.mkdir();(runtime/'data').symlink_to(destination,target_is_directory=True)
        lab.root=runtime;lab.data=destination
        assert len(lab.run('password-audit')['events'])==count
        lab.start();login(lab,1,bad,ok=False);barrier(login(lab,1,good));lab.stop()
    lab.root=root;lab.data=source;assert hashes(backup)==saved
    for file in source.glob('rgg.sqlite*'):
        assert PASSWORD not in file.read_bytes() and NEW not in file.read_bytes()
    lab.run('retire','--uin','1');assert lab.run('password-audit')['events'][0]['uin']==1

def refusal(lab):
    before=lab.sql('SELECT credential_version,nonce,ciphertext FROM accounts WHERE uin=1')
    for old,new in [(b'WrongTest9!',NEW),(PASSWORD,PASSWORD),(PASSWORD,b'short'),(PASSWORD,b'1234567890123456'),(PASSWORD,'Zolcą12345'.encode()),(PASSWORD,b'has a space')]:change(lab,old,new,ok=False)
    for raw in [b'{}',b'{bad',b'x'*4097,json.dumps({'current_password':PASSWORD.decode(),'new_password':NEW.decode(),'unexpected':'rejected'}).encode()]:change(lab,raw=raw,ok=False)
    change(lab,uin=555,ok=False);lab.run('retire','--uin','3');change(lab,uin=3,ok=False)
    assert lab.sql('SELECT credential_version,nonce,ciphertext FROM accounts WHERE uin=1')==before and not lab.run('password-audit')['events']
    pending=lab.data/'keys.pending';pending.write_bytes((lab.data/'keys.toml').read_bytes());pending.chmod(0o600)
    assert b'pending key rotation' in change(lab,ok=False).stderr;pending.unlink()
    key=lab.data/'keys.toml';saved=lab.data/'keys.saved';key.rename(saved)
    change(lab,ok=False);assert not key.exists();saved.rename(key)
    data=lab.data;lab.data=lab.root/'uninitialized';change(lab,ok=False);assert not lab.data.exists();lab.data=data
    helper=Path(__file__).resolve().parents[1]/'tools/change_password.py'
    p=subprocess.run([sys.executable,str(helper),'--admin',str(lab.admin),'--data-dir',str(data),'--uin','1'],input=b'',capture_output=True,timeout=10)
    assert p.returncode!=0 and PASSWORD not in p.stdout+p.stderr

def crash(lab,stage):
    before=lab.sql('SELECT count(*) FROM secret_nonces')[0][0]
    p=change(lab,ok=False,fault=stage);assert p.returncode==86
    committed=stage=='after_password_commit'
    assert lab.sql('SELECT credential_version FROM accounts WHERE uin=1')==[(2 if committed else 1,)]
    assert lab.sql('SELECT count(*) FROM secret_nonces')==[(before+int(committed),)]
    assert len(lab.run('password-audit')['events'])==int(committed)
    lab.start();login(lab,1,PASSWORD if committed else NEW,ok=False);barrier(login(lab,1,NEW if committed else PASSWORD));lab.stop()
    assert lab.sql('PRAGMA integrity_check')==[('ok',)]

def migration(server,admin,root,baseline):
    lab=Lab(baseline/'rgg-server',baseline/'rgg-admin',root)
    try:
        prepare(lab);assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(10,)]
        snapshot=root/'schema10';create(lab,snapshot);lab.binary=server;lab.admin=admin
        assert change(lab)['credential_version']==2 and lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(12,)]
        lab.start();b=lab.client(2);text(b);a=login(lab,1,NEW);barrier(a);download(a,b);lab.stop()
        restored=root/'restored10';p=subprocess.run([str(baseline/'rgg-admin'),'restore','--backup',str(snapshot),'--data-dir',str(restored)],capture_output=True,timeout=15);assert p.returncode==0,p.stderr
        lab.data=restored;assert change(lab)['credential_version']==2
        assert len(lab.run('password-audit')['events'])==1
    finally:lab.stop()

def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);p.add_argument('--faults',action='store_true');p.add_argument('--baseline-dir',type=Path);a=p.parse_args()
    cases=[('new/old GG6 login, retained messages/images/profile/contacts, audit and independent backups',normal),('bad credentials/input, active server, pending rotation and missing data/key refused',refusal)]
    if a.faults:
        for stage in ['before_password_commit','after_password_commit']:cases.append((stage,lambda lab,stage=stage:crash(lab,stage)))
    for name,test in cases:
        with tempfile.TemporaryDirectory(prefix='rgg-password-') as root:
            lab=Lab(a.server.resolve(),a.admin.resolve(),Path(root))
            try:test(lab);print('PASS:',name,flush=True)
            finally:lab.stop()
    if a.baseline_dir:
        with tempfile.TemporaryDirectory(prefix='rgg-password-migration-') as root:
            migration(a.server.resolve(),a.admin.resolve(),Path(root),a.baseline_dir.resolve());print('PASS: real dev.15 schema-10 migration/restore, queued image and password change',flush=True)
if __name__=='__main__':main()
