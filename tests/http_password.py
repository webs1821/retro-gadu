# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Full-password self-service on synthetic data: HTTP, live GG6 sessions, backups and crashes."""
import argparse,concurrent.futures,http.client,re,sqlite3,subprocess,tempfile,urllib.parse
from pathlib import Path
from gg6_sessions import Lab,PASSWORD
from http_discovery import request
from http_registration import form as registration_form,fields as registration_fields,submit as registration_submit
from gg6_presence_contacts import barrier,rules,get_archive
from gg6_directory import query
from gg6_images import text,download
from backups_cli import prepare,create,restore,hashes
from password_cli import login,change as offline_change
NEW=b'NewLocalTest8!'

def form(lab):
    code,headers,body=request(lab,'/password');assert code==200,(code,body)
    assert 'Path=/password' in headers['set-cookie'] and 'HttpOnly' in headers['set-cookie'] and 'SameSite=Strict' in headers['set-cookie']
    assert headers['cache-control']=='no-store' and 'script-src' not in headers['content-security-policy']
    csrf=re.search(rb'name="csrf" value="([0-9a-f]{64})"',body).group(1).decode()
    return csrf,headers['set-cookie'].split(';',1)[0]
def values(csrf,uin=1,old=PASSWORD,new=NEW):
    return dict(csrf=csrf,uin=str(uin),current_password=old.decode(),new_password=new.decode(),confirm_password=new.decode())
def submit(lab,data,cookie,headers=None):
    base={'Origin':'http://'+lab.server.address,'Content-Type':'application/x-www-form-urlencoded','Cookie':cookie}
    for name,value in (headers or {}).items():
        if value is None:base.pop(name,None)
        else:base[name]=value
    result=request(lab,'/password',method='POST',body=urllib.parse.urlencode(data),headers=base)
    for secret in [data.get('current_password',''),data.get('new_password','')]:
        if len(secret)>7:assert secret.encode() not in result[2]
    return result
def change(lab,**kwargs):
    csrf,cookie=form(lab);return submit(lab,values(csrf,**kwargs),cookie)
def audit(lab):return lab.sql('SELECT actor,target_uin,previous_version,credential_version FROM admin_audit ORDER BY id')

def normal(lab):
    prepare(lab);before=lab.root/'before';create(lab,before);saved=hashes(before)
    lab.start(passwords=True,statistics=True)
    a=lab.client(1);barrier(a);rules(a,[(3,4)]);assert a.message(2,42,'oczekuje po zmianie')==3
    fields=['SELECT * FROM contact_archives','SELECT * FROM contact_rules','SELECT * FROM public_profiles','SELECT * FROM invitations','SELECT * FROM image_offers']
    snapshot={q:lab.sql(q) for q in fields}
    assert change(lab,old=b'WrongTest9!')[0]==403;a.ping()
    assert change(lab,new=PASSWORD)[0]==400;a.ping();assert not audit(lab)
    code,headers,_=change(lab);assert code==200 and 'Max-Age=0' in headers['set-cookie'];a.closed()
    assert lab.sql('SELECT count(*) FROM account_sessions WHERE uin=1')==[(0,)]
    assert {q:lab.sql(q) for q in fields}==snapshot
    assert audit(lab)==[('local_web_with_current_password',1,1,2)]
    assert b'href="/password"' in request(lab,'/')[2]
    login(lab,1,PASSWORD,ok=False)
    b=lab.client(2);text(b);assert b.receive()[1][16:].startswith('oczekuje po zmianie'.encode())
    a=login(lab,1,NEW);barrier(a);assert query(a,2)['firstname']=='Próba kopii';assert get_archive(a)=='Znajomy;2\r\n'.encode('cp1250');download(a,b)
    lab.stop();assert lab.run('password-audit')['events'][0]['actor']=='local_web_with_current_password'
    after=lab.root/'after';create(lab,after)
    source,root=lab.data,lab.root
    for name,backup,good,bad,events in [('old',before,PASSWORD,NEW,0),('new',after,NEW,PASSWORD,1)]:
        destination=root/(name+'-restored');restore(lab,backup,destination)
        runtime=root/(name+'-runtime');runtime.mkdir();(runtime/'data').symlink_to(destination,target_is_directory=True)
        lab.root,lab.data=runtime,destination
        assert len(lab.run('password-audit')['events'])==events
        lab.start(passwords=True);login(lab,1,bad,ok=False);barrier(login(lab,1,good));lab.stop()
    lab.root,lab.data=root,source;assert hashes(before)==saved
    for file in source.glob('rgg.sqlite*'):
        assert PASSWORD not in file.read_bytes() and NEW not in file.read_bytes()
    assert NEW not in (root/'server.log').read_bytes()

def guards(lab):
    lab.start(statistics=True);assert request(lab,'/password')[0]==404 and b'href="/password"' not in request(lab,'/')[2];lab.stop()
    lab.start(passwords=True,registration=True)
    csrf,cookie=form(lab);data=values(csrf)
    for bad in [{'Origin':None},{'Origin':'https://attacker.invalid'},{'Host':'attacker.invalid'},{'Sec-Fetch-Site':'cross-site'},{'Cookie':cookie+'; '+cookie}]:assert submit(lab,data,cookie,bad)[0]==403
    assert submit(lab,{**data,'csrf':'00'*32},cookie)[0]==403
    assert submit(lab,{**data,'unknown':'forbidden'},cookie)[0]==400
    assert submit(lab,{**data,'new_password':'SYNTHETIC_CANARY'*600},cookie)[0]==413
    assert request(lab,'/password?current_password=SYNTHETIC_CANARY')[0]==403
    register_token,register_cookie=registration_form(lab)
    assert submit(lab,values(register_token),'rgg_password='+register_token)[0]==403
    assert registration_submit(lab,registration_fields(csrf,'00'*16),'rgg_register='+csrf)[0]==403
    for edits in [{'uin':'0'},{'uin':'16777216'},{'uin':'+1'},{'new_password':'short'},{'new_password':'zażółć123'},{'confirm_password':'Different9!'}]:
        token,new_cookie=form(lab);assert submit(lab,{**values(token),**edits},new_cookie)[0]==400
    # Cross-purpose failures do not consume the valid password token.
    assert submit(lab,data,cookie)[0]==200
    assert submit(lab,data,cookie)[0]==403
    assert len(audit(lab))==1
    lab.stop();config=lab.root/'bad.toml'
    config.write_text("data_dir='not-created'\nhttp_bind='127.0.0.1:0'\nweb_password_change=true\n")
    p=subprocess.run([str(lab.binary),'--config',str(config)],capture_output=True,timeout=5);assert p.returncode!=0 and not (lab.root/'not-created').exists()
    (lab.data/'keys.pending').write_bytes(b'SYNTHETIC_PENDING_MARKER')
    p=subprocess.run([str(lab.binary),'--config',str(lab.root/'rgg.toml')],capture_output=True,timeout=5);assert p.returncode!=0 and b'pending key rotation' in p.stderr

def budget_and_offline(lab):
    lab.start(passwords=True);assert request(lab,'/')[0]==404;a=lab.client(1)
    responses=[change(lab,old=b'WrongTest9!') for _ in range(5)]
    assert all(r[0]==403 for r in responses);assert change(lab)[0:3:2]==responses[0][0:3:2];a.ping()
    assert change(lab,uin=999)[0:3:2]==responses[0][0:3:2]
    assert lab.sql('SELECT uin,attempts FROM password_change_budget')==[(1,5)] and not audit(lab)
    lab.stop();lab.start(passwords=True);assert change(lab)[0]==403
    b=lab.client(2);assert b.message(1,44,'offline zachowane')==3
    lab.stop()
    # Only synthetic fixture clock: expire the persisted window; do not wait ten minutes.
    with sqlite3.connect(lab.data/'rgg.sqlite') as db:db.execute('UPDATE password_change_budget SET window_started=window_started-600')
    lab.start(passwords=True);assert change(lab)[0]==200
    a=login(lab,1,NEW);kind,body=a.receive();assert kind==10 and body[16:]==b'offline zachowane\0';a.ping()
    assert lab.sql('SELECT attempts FROM password_change_budget')==[(1,)]

def concurrency(lab):
    lab.start(passwords=True);first=form(lab);second=form(lab)
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        a=pool.submit(submit,lab,values(first[0]),first[1]);b=pool.submit(submit,lab,values(second[0],new=b'OtherLocal9!'),second[1]);results=[a.result(),b.result()]
    assert sorted(r[0] for r in results)==[200,403]
    good=NEW if results[0][0]==200 else b'OtherLocal9!'
    assert len(audit(lab))==1;login(lab,1,PASSWORD,ok=False);barrier(login(lab,1,good))

def crash(lab,stage):
    before=lab.sql('SELECT count(*) FROM secret_nonces')[0][0]
    lab.start(passwords=True,password_fault=stage);a=lab.client(1)
    try:change(lab);raise AssertionError('expected interrupted HTTP response')
    except (http.client.RemoteDisconnected,ConnectionResetError):pass
    lab.server.process.wait(timeout=5);assert lab.server.process.returncode==86;a.closed()
    committed=stage=='after_web_password_commit'
    assert lab.sql('SELECT credential_version FROM accounts WHERE uin=1')==[(2 if committed else 1,)]
    assert len(audit(lab))==int(committed)
    assert lab.sql('SELECT count(*) FROM secret_nonces')==[(before+int(committed),)]
    assert lab.sql('PRAGMA integrity_check')==[('ok',)] and not lab.sql('PRAGMA foreign_key_check')
    lab.stop();lab.start(passwords=True)
    login(lab,1,PASSWORD if committed else NEW,ok=False);barrier(login(lab,1,NEW if committed else PASSWORD))

def migration(server,admin,baseline,root,empty):
    lab=Lab(baseline/'rgg-server',baseline/'rgg-admin',root)
    try:
        prepare(lab);offline_change(lab);offline_change(lab,old=NEW,new=PASSWORD)
        assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(11,)]
        with sqlite3.connect(lab.data/'rgg.sqlite') as db:
            db.execute('DELETE FROM admin_audit' if empty else 'DELETE FROM admin_audit WHERE id=2')
        original=lab.sql('SELECT * FROM admin_audit');assert lab.sql("SELECT seq FROM sqlite_sequence WHERE name='admin_audit'")==[(2,)]
        backup=root/'old-backup';create(lab,backup);saved=hashes(backup)
        lab.binary,lab.admin=server,admin
        lab.start(passwords=True)
        assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(14,)]
        assert lab.sql('SELECT * FROM admin_audit')==original
        assert change(lab)[0]==200
        assert lab.sql('SELECT id,actor,credential_version FROM admin_audit WHERE id>2')==[(3,'local_web_with_current_password',4)]
        assert lab.sql('PRAGMA integrity_check')==[('ok',)] and not lab.sql('PRAGMA foreign_key_check')
        b=lab.client(2);text(b);a=login(lab,1,NEW);barrier(a);download(a,b);lab.stop()
        destination=root/'restored11';lab.admin=baseline/'rgg-admin';restore(lab,backup,destination);lab.admin=admin
        runtime=root/'restored-runtime';runtime.mkdir();(runtime/'data').symlink_to(destination,target_is_directory=True);lab.root,lab.data=runtime,destination
        lab.start(passwords=True);assert change(lab)[0]==200
        assert lab.sql('SELECT id,actor FROM admin_audit WHERE id>2')==[(3,'local_web_with_current_password')]
        assert hashes(backup)==saved
        login(lab,1,PASSWORD,ok=False);barrier(login(lab,1,NEW))
    finally:lab.stop()

def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);p.add_argument('--faults',action='store_true');p.add_argument('--baseline-dir',type=Path);args=p.parse_args()
    cases=[('live session fence, data preservation, new login and backup/restore',normal),('opt-in, independent CSRF purpose, Host/Origin, malformed forms and key-rotation gate',guards),('persistent account budget, uniform rejection and offline replay',budget_and_offline),('competing current-password changes commit once',concurrency)]
    if args.faults:
        for stage in ['before_web_password_commit','after_web_password_commit']:cases.append((stage,lambda lab,stage=stage:crash(lab,stage)))
    for name,test in cases:
        with tempfile.TemporaryDirectory(prefix='rgg-http-password-') as tmp:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(tmp))
            try:test(lab);print('PASS:',name,flush=True)
            finally:lab.stop()
    if args.baseline_dir:
        for empty in [False,True]:
            with tempfile.TemporaryDirectory(prefix='rgg-password-migration-') as tmp:
                migration(args.server.resolve(),args.admin.resolve(),args.baseline_dir.resolve(),Path(tmp),empty)
                print('PASS: real schema 11→14, old backup restore and audit high-water mark (empty='+str(empty)+')',flush=True)
if __name__=='__main__':main()
