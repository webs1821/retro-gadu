# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Valid GG6 commands in disallowed states; real loopback TCP and synthetic data only."""
import argparse,contextlib,hashlib,json,sqlite3,tempfile,time
from pathlib import Path
from gg6_sessions import Lab,words,PASSWORD,oracle,until
from gg6_presence_contacts import barrier,rules,put_archive,get_archive,status
from gg6_directory import query

CANARY=b'StateRejectedCanary'
ARCHIVE=b'OriginalStateArchive'
TABLES=('messages','deliveries','image_offers','public_profiles','contact_archives','contact_rules','directory_rate')

def sql(lab,statement):
    with contextlib.closing(sqlite3.connect(lab.data/'rgg.sqlite')) as db:
        return db.execute(statement).fetchall()

def snapshot(lab):
    with contextlib.closing(sqlite3.connect(lab.data/'rgg.sqlite')) as db:
        db.execute('BEGIN')
        return {name:db.execute('SELECT * FROM '+name+' ORDER BY 1').fetchall() for name in TABLES}

def probes():
    message=words(2,77,8)+CANARY+b'\0'
    image=words(2,78,4)+bytes([0])
    return [
        ('text',0xb,message),
        ('conference',0xb,message+bytes([1])+words(1,1)),
        ('image-offer',0xb,words(2,79,8)+b'\xa0\0'+bytes([2,13,0,0,0,128,9,1])+words(3,123)),
        ('image-request',0xb,image+bytes([4])+words(3,123)),
        ('image-start',0xb,image+bytes([5])+words(3,123)+b'a.png\0x'),
        ('directory-own',0x14,bytes([2])+words(0)),
        ('directory-write',0x14,bytes([1])+words(0)+b'nickname\0'+CANARY+b'\0'),
        ('directory-search',0x14,bytes([3])+words(77)+b'nickname\0Original\0'),
        ('archive-get',0x16,bytes([2])),
        ('archive-put',0x16,b'\0'+CANARY),
        ('contact-add',0xd,words(2)+bytes([4])),
        ('contact-remove',0xe,words(1)+bytes([4])),
    ]

def closed_without_effects(lab,client,observer,original):
    client.closed();client.close()
    until(lambda:sql(lab,'SELECT count(*) FROM account_sessions WHERE uin=3')==[(0,)])
    assert snapshot(lab)==original,'disallowed command changed durable user state'
    assert sql(lab,'SELECT count(*) FROM contact_staging')==[(0,)]
    observer.ping()
    assert observer.message(3,90,'StillBlocked')==6
    assert snapshot(lab)==original
    assert CANARY not in lab.server.log.read_bytes()

def matrix(lab):
    lab.start();observer=lab.client(1)
    owner=lab.client(3,ready=False);rules(owner,[(1,4)])
    put_archive(owner,ARCHIVE);query(owner,1,{'nickname':'Original'})
    owner.close()
    until(lambda:sql(lab,'SELECT count(*) FROM account_sessions WHERE uin=3')==[(0,)])
    original=snapshot(lab);results=[]
    for phase in ('unauthenticated','initial','resynchronizing'):
        commands=probes()
        if phase=='unauthenticated':commands += [('ping',8,b''),('status',2,words(2)),('empty-notify',0x12,b'')]
        else:commands += [('repeat-login',0x15,None)]
        if phase=='resynchronizing':commands += [('empty-during-staging',0x12,b'')]
        for name,kind,body in commands:
            client=lab.client()
            if phase!='unauthenticated':
                assert client.login(3,ready=False)==(3,b'')
                if phase=='resynchronizing':
                    rules(client,[(1,4)])
                    # Proposed unblocking is only staging; the committed block must survive.
                    client.send(0xf,words(1)+bytes([3]));barrier(client)
                assert sql(lab,'SELECT ready FROM account_sessions WHERE uin=3')==[(0,)]
            if body is None:
                body=words(3,oracle(PASSWORD,client.seed),2,0x20)+bytes(13)+bytes([255,0xbe])
            client.send(kind,body)
            if phase=='unauthenticated':assert client.receive()==(9,b'')
            closed_without_effects(lab,client,observer,original)
            results.append({'phase':phase,'command':name})
    owner=lab.client(3,ready=False)
    status(owner,4,'PendingDescription');barrier(owner)
    assert sql(lab,'SELECT count(*) FROM account_presence WHERE uin=3')==[(0,)]
    assert snapshot(lab)==original
    rules(owner,[(1,4)])
    assert sql(lab,'SELECT description FROM account_presence WHERE uin=3')==[('PendingDescription',)]
    assert get_archive(owner)==ARCHIVE and query(owner,2)['nickname']=='Original'
    recipient=lab.client(2)
    assert observer.message(2,123,'AfterStateChecks')==3
    assert recipient.receive()[1][16:]==b'AfterStateChecks\0'
    lab.stop();assert sql(lab,'PRAGMA integrity_check')==[('ok',)]
    assert sql(lab,'PRAGMA foreign_key_check')==[]
    result={'cases':results,'count':len(results),'state_preserved':True,'legal_sync_and_message_after_checks':True}
    print('SESSION_STATE_REPORT='+json.dumps(result),flush=True)
    return result

def initial_deadline(lab):
    lab.start();observer=lab.client(1);client=lab.client(3,ready=False)
    started=time.monotonic();pongs=0
    while time.monotonic()-started<2.5:
        try:client.ping();pongs+=1
        except (EOFError,ConnectionResetError,BrokenPipeError):break
        time.sleep(.1)
    else:raise AssertionError('pings extended the absolute initial-state deadline')
    elapsed=time.monotonic()-started
    assert pongs>1 and .8<=elapsed<2.5,(pongs,elapsed)
    client.close();observer.ping()
    until(lambda:sql(lab,'SELECT count(*) FROM account_sessions WHERE uin=3')==[(0,)])
    assert sql(lab,'SELECT count(*) FROM messages')==[(0,)]
    print('PASS: pings remain legal during synchronization but cannot extend its deadline',flush=True)
    return {'pongs':pongs,'closed_after_seconds':round(elapsed,3),'unrelated_session_works':True}

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--server',type=Path,required=True);parser.add_argument('--admin',type=Path,required=True);parser.add_argument('--report',type=Path);args=parser.parse_args()
    if args.report:assert not args.report.exists() and args.report.parent.is_dir()
    def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
    report={'format':1,'passed':False,'server_sha256':sha(args.server),'admin_sha256':sha(args.admin),
            'driver_sha256':sha(Path(__file__)),'checks':[],'native_client_qualified':False,'pushed':False,'deployed':False}
    try:
        for test in (matrix,initial_deadline):
            with tempfile.TemporaryDirectory(prefix='rgg-session-contracts-') as root:
                lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root));started=time.monotonic()
                try:result=test(lab)
                finally:lab.stop()
            report['checks'].append({'test':test.__name__,'seconds':round(time.monotonic()-started,3),'result':result})
        report['passed']=True
    finally:
        if args.report:
            with args.report.open('x',encoding='utf-8') as handle:json.dump(report,handle,indent=2);handle.write('\n')

if __name__=='__main__':main()
