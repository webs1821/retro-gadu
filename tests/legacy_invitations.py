# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Synthetic legacy invite hashes/quotas via CLI and HTTP, plus real schema-8 migration."""
import argparse,concurrent.futures,hashlib,json,sqlite3,subprocess,tempfile
from pathlib import Path
from gg6_sessions import Lab,PASSWORD
from http_registration import form,fields,submit,created,login
from backups_cli import prepare,create,verify,restore
from gg6_images import text,download
from gg6_presence_contacts import barrier
def seed(lab,code,used=0,active=1):
    db=sqlite3.connect(str(lab.data/'rgg.sqlite'))
    try:
        with db:db.execute('INSERT INTO legacy_invitations VALUES(?,3,?,?,1000)',(hashlib.sha256(code.encode()).digest(),used,active))
    finally:db.close()
def account(lab,code,uin=None,ok=True):
    payload={'requested_uin':uin,'invite':code,'password':PASSWORD.decode(),'email':'synthetic@example.invalid'}
    p=subprocess.run([str(lab.admin),'account-create','--data-dir',str(lab.data)],input=json.dumps(payload).encode(),capture_output=True,timeout=10)
    assert code.encode() not in p.stdout+p.stderr and PASSWORD not in p.stdout+p.stderr
    if ok:assert p.returncode==0,p.stderr;return json.loads(p.stdout)['uin']
    assert p.returncode!=0
def cli(lab):
    code='CaseSensitive-Legacy_Code42';seed(lab,code,used=1)
    account(lab,code.lower(),42,ok=False);account(lab,code,1,ok=False)
    assert lab.sql('SELECT used FROM legacy_invitations')==[(1,)]
    assert account(lab,code,42)==42
    lab.sql('UPDATE legacy_invitations SET active=0');account(lab,code,43,ok=False)
    assert lab.sql('SELECT used FROM legacy_invitations')==[(2,)]
    lab.sql('UPDATE legacy_invitations SET active=1');assert account(lab,code,43)==43;account(lab,code,44,ok=False)
    assert lab.sql('SELECT used FROM legacy_invitations')==[(3,)]
    lab.start();lab.client(42).ping();lab.client(43).ping();lab.stop()
    backup=lab.root/'legacy-backup';create(lab,backup);restored=lab.root/'restored';restore(lab,backup,restored)
    lab.data=restored;assert lab.sql('SELECT used,active FROM legacy_invitations')==[(3,1)];account(lab,code,44,ok=False)
def web(lab):
    code='A'*126+'-_';seed(lab,code,used=2);lab.start(registration=True)
    prepared=[form(lab) for _ in range(12)]
    def register(item):token,cookie=item;return submit(lab,fields(token,code),cookie)
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:results=list(pool.map(register,prepared))
    successful=[created(r) for r in results if r[0]==201];assert successful==[112233],[r[0] for r in results]
    assert all(r[0] in (201,400,429) for r in results)
    assert lab.sql('SELECT used FROM legacy_invitations')==[(3,)] and lab.sql('SELECT sum(used) FROM invitations')==[(3,)]
    login(lab,112233);assert code.encode() not in lab.server.log.read_bytes()
def modern_priority(lab):
    code=lab.run('invite','--uses','1')['codes'][0];seed(lab,code)
    assert account(lab,code,44)==44;account(lab,code,45,ok=False);account(lab,code.upper(),45,ok=False)
    assert lab.sql('SELECT used FROM legacy_invitations')==[(0,)] and lab.sql('SELECT sum(used) FROM invitations')==[(4,)]
def migration(lab,server,admin):
    prepare(lab);assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(8,)]
    accounts=lab.sql('SELECT * FROM accounts');old_admin=lab.admin
    backup=lab.root/'old-backup';create(lab,backup)
    lab.binary=server;lab.admin=admin;lab.run('stats')
    assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(11,)]
    assert lab.sql('SELECT count(*) FROM legacy_invitations')==[(0,)] and lab.sql('SELECT * FROM accounts')==accounts
    assert lab.sql('SELECT used FROM invitations')==[(3,)] and lab.sql('SELECT uin FROM retired_uins')==[(3,)]
    verify(lab,backup,ok=False) # new verifier deliberately requires its exact current schema
    restored=lab.root/'old-restore';p=subprocess.run([str(old_admin),'restore','--backup',str(backup),'--data-dir',str(restored)],capture_output=True,timeout=10);assert p.returncode==0,p.stderr
    p=subprocess.run([str(admin),'stats','--data-dir',str(restored)],capture_output=True,timeout=10);assert p.returncode==0 and json.loads(p.stdout)['registered']==2
    lab.start();b=lab.client(2);text(b);a=lab.client(1);barrier(a);download(a,b)
def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);p.add_argument('--baseline-server',type=Path);p.add_argument('--baseline-admin',type=Path);a=p.parse_args()
    cases=[('case-sensitive legacy usage, rollback, revocation, exhaustion and restore',cli),('128-character code, 12 concurrent HTTP attempts, exactly one remaining use',web),('modern code never falls through to legacy after exhaustion',modern_priority)]
    for name,test in cases:
        with tempfile.TemporaryDirectory(prefix='rgg-legacy-invites-') as root:
            lab=Lab(a.server.resolve(),a.admin.resolve(),Path(root))
            try:test(lab);print('PASS:',name,flush=True)
            finally:lab.stop()
    if a.baseline_server and a.baseline_admin:
        with tempfile.TemporaryDirectory(prefix='rgg-invite-migrate-') as root:
            lab=Lab(a.baseline_server.resolve(),a.baseline_admin.resolve(),Path(root))
            try:migration(lab,a.server.resolve(),a.admin.resolve());print('PASS: real dev.13 migration and schema-8 restore through old binary, text/images preserved',flush=True)
            finally:lab.stop()
if __name__=='__main__':main()
