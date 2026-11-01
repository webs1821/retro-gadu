# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Complete offline snapshots, immutable verification, guarded restore and process crashes."""
import argparse,hashlib,json,os,shutil,sqlite3,subprocess,tempfile
from pathlib import Path
from gg6_sessions import Lab,PASSWORD
from gg6_presence_contacts import barrier,rules,put_archive,get_archive
from gg6_directory import query
from gg6_images import offer,text,download
def cmd(lab,*args,ok=True,fault=None):
    env=os.environ.copy();env.pop('RGG_TEST_BACKUP_CRASH',None)
    if fault:env['RGG_TEST_BACKUP_CRASH']=fault
    p=subprocess.run([str(lab.admin),*map(str,args)],capture_output=True,timeout=15,env=env)
    assert PASSWORD not in p.stdout+p.stderr
    if ok:assert p.returncode==0,p.stderr;return json.loads(p.stdout)
    assert p.returncode!=0;return p
def create(lab,path,**kw):return cmd(lab,'backup-create','--data-dir',lab.data,'--output',path,**kw)
def verify(lab,path,**kw):return cmd(lab,'backup-verify','--backup',path,**kw)
def restore(lab,backup,path,**kw):return cmd(lab,'restore','--backup',backup,'--data-dir',path,**kw)
def hashes(root):return {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in root.iterdir() if p.is_file()}
def rehash(root):
    doc=json.loads((root/'manifest.json').read_text());data=(root/'rgg.sqlite').read_bytes();doc['database']={'size':len(data),'sha256':hashlib.sha256(data).hexdigest()};(root/'manifest.json').write_text(json.dumps(doc))
def prepare(lab):
    lab.run('retire','--uin','3');lab.start();a=lab.client(1);barrier(a)
    query(a,1,{'firstname':'Próba kopii','nickname':'synthetic'})
    put_archive(a,'Znajomy;2\r\n'.encode('cp1250'));rules(a,[(3,4)])
    offer(a);lab.stop(hard=True)
def roundtrip(lab):
    prepare(lab);backup=lab.root/'snapshot';created=create(lab,backup);assert created['verification']['accounts']==2
    assert set(p.name for p in backup.iterdir())=={'manifest.json','rgg.sqlite','keys.toml'}
    if os.name=='posix':
        assert backup.stat().st_mode&0o777==0o700
        assert all(p.stat().st_mode&0o777==0o600 for p in backup.iterdir())
    before=hashes(backup);assert verify(lab,backup)['verified'];assert hashes(backup)==before
    restored=lab.root/'restored';assert restore(lab,backup,restored)['restored'];assert not (restored/'restore-incomplete').exists()
    # Point only this synthetic fixture at the restored data. Source stays intact.
    source=lab.data;lab.data=restored
    assert lab.run('stats')['registered']==2
    assert lab.sql('SELECT uin FROM retired_uins')==[(3,)]
    assert lab.sql('SELECT peer,flags FROM contact_rules WHERE owner=1')==[(3,4)]
    assert lab.sql('SELECT count(*) FROM image_offers')==[(1,)]
    assert lab.sql('SELECT used FROM invitations')==[(3,)]
    # Lab configuration uses root/data; select an isolated runtime root containing a data alias.
    runtime=lab.root/'runtime';runtime.mkdir();(runtime/'data').symlink_to(restored,target_is_directory=True);lab.root=runtime
    lab.start();b=lab.client(2);text(b);a=lab.client(1);barrier(a)
    assert query(a,2)['firstname']=='Próba kopii';assert get_archive(a)=='Znajomy;2\r\n'.encode('cp1250');download(a,b)
    lab.stop();assert hashes(backup)==before
    assert (source/'keys.toml').read_bytes()==(restored/'keys.toml').read_bytes()
def refusal(lab):
    backup=lab.root/'snapshot';create(lab,backup)
    before=hashes(backup);assert b'destination' in create(lab,backup,ok=False).stderr;assert hashes(backup)==before
    target=lab.root/'existing';target.mkdir();(target/'sentinel').write_text('KEEP');restore(lab,backup,target,ok=False);assert (target/'sentinel').read_text()=='KEEP'
    lab.start();live=lab.root/'must-not-create';p=create(lab,live,ok=False);assert b'another process' in p.stderr and not live.exists();lab.stop()
    missing=lab.root/'missing-source';cmd(lab,'backup-create','--data-dir',missing,'--output',live,ok=False);assert not missing.exists() and not live.exists()
    cases=['dbhash','missingkey','wrongkey','credential','migration','foreign-key','wal','symlink','permissions']
    for kind in cases:
        bad=lab.root/('bad-'+kind);shutil.copytree(backup,bad)
        if kind=='dbhash':
            with (bad/'rgg.sqlite').open('ab') as f:f.write(b'changed')
        elif kind=='missingkey':(bad/'keys.toml').unlink()
        elif kind=='wrongkey':
            another=lab.root/'different-keys';cmd(lab,'init','--data-dir',another);shutil.copyfile(another/'keys.toml',bad/'keys.toml');doc=json.loads((bad/'manifest.json').read_text());key=(bad/'keys.toml').read_bytes();doc['keys']={'size':len(key),'sha256':hashlib.sha256(key).hexdigest()};(bad/'manifest.json').write_text(json.dumps(doc))
        elif kind in ('credential','migration','foreign-key'):
            with sqlite3.connect(str(bad/'rgg.sqlite')) as db:
                if kind=='credential':db.execute('UPDATE accounts SET ciphertext=zeroblob(length(ciphertext)) WHERE uin=2') # not the per-key first row
                elif kind=='migration':db.execute('UPDATE _sqlx_migrations SET checksum=zeroblob(48) WHERE version=8')
                else:db.execute('DELETE FROM accounts WHERE uin=1') # nonce history FK is deliberately broken
            rehash(bad)
        elif kind=='wal':(bad/'rgg.sqlite-wal').write_bytes(b'not a standalone snapshot')
        elif kind=='symlink':(bad/'keys.toml').unlink();(bad/'keys.toml').symlink_to(backup/'keys.toml')
        else:(bad/'keys.toml').chmod(0o644)
        rejected=verify(lab,bad,ok=False);assert rejected.returncode!=0
        dest=lab.root/('no-restore-'+kind);restore(lab,bad,dest,ok=False);assert not dest.exists()
def crashes(lab):
    pending=lab.root/'pending';p=create(lab,pending,ok=False,fault='before_backup_commit');assert p.returncode==86
    assert not (pending/'manifest.json').exists() and (pending/'manifest.pending').exists();verify(lab,pending,ok=False)
    backup=lab.root/'good';create(lab,backup)
    target=lab.root/'interrupted';p=restore(lab,backup,target,ok=False,fault='before_restore_commit');assert p.returncode==86 and (target/'restore-incomplete').exists()
    p=cmd(lab,'stats','--data-dir',target,ok=False);assert b'incomplete restore' in p.stderr
    config=lab.root/'blocked.toml';config.write_text("data_dir='interrupted'\nhttp_bind='127.0.0.1:0'\n")
    p=subprocess.run([str(lab.binary),'--config',str(config)],capture_output=True,timeout=5);assert p.returncode!=0 and b'incomplete restore' in p.stderr
    fresh=lab.root/'fresh';restore(lab,backup,fresh);assert cmd(lab,'stats','--data-dir',fresh)['registered']==3
def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);p.add_argument('--faults',action='store_true');a=p.parse_args()
    cases=[('complete snapshot and authenticated restored text/images/profile/contacts/invites',roundtrip),('existing/live destinations and corrupt/incomplete/wrong-key backups refused',refusal)]
    if a.faults:cases.append(('interrupted publication and restore fail closed; clean destination recovers',crashes))
    for name,test in cases:
        with tempfile.TemporaryDirectory(prefix='rgg-backup-') as root:
            lab=Lab(a.server.resolve(),a.admin.resolve(),Path(root))
            try:test(lab);print('PASS:',name,flush=True)
            finally:lab.stop()
if __name__=='__main__':main()
