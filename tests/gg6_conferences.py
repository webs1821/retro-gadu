# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Independent legacy conference wire tests; client fans out, server routes one copy.
Protocol facts: libgadu-wire-reference. GG32 oracle is inherited from gg6_sessions.
"""
import argparse,struct,tempfile
from pathlib import Path
from gg6_sessions import Lab,words,PASSWORD,until
from gg6_presence_contacts import barrier,rules

STYLE=bytes([2,6,0,0,0,9,255,0,127])
def context(others):return b"\1"+words(len(others),*others)
def fourth(lab):
    code=lab.run("invite")["codes"][0]
    lab.run("account-create",payload={"requested_uin":4,"invite":code,"password":PASSWORD.decode(),"email":"synthetic@example.invalid"})
def received(client,others,text="Żółć",styled=True,queued=None):
    kind,body=client.receive();assert kind==10
    sender,seq,stamp,flags=struct.unpack("<IIII",body[:16]);assert sender==1 and seq==77 and stamp>0
    if queued is not None:assert bool(flags&1)==queued
    assert body[16:]==text.encode("cp1250")+b"\0"+context(others)+(STYLE if styled else b""),body
def four_people(lab):
    fourth(lab);lab.start();a=lab.client(1);b=lab.client(2);d=lab.client(4)
    for client in [a,b,d]:barrier(client)
    for target,others in [(2,[4,3]),(3,[2,4]),(4,[3,2])]:
        assert a.message(target,77,options=context(others)+STYLE)==3
    received(b,[4,3],queued=False);received(d,[3,2],queued=False)
    until(lambda:lab.sql("SELECT count(*) FROM deliveries WHERE state='Pending'")==[(1,)])
    assert lab.sql("SELECT recipient,count(*) FROM messages GROUP BY recipient ORDER BY recipient")==[(2,1),(3,1),(4,1)]
    for client in [a,b,d]:client.quiet()
    lab.stop(hard=True);lab.start();c=lab.client(3);received(c,[2,4],queued=True)
    c.ping();c.quiet();lab.client(2).quiet();lab.client(4).quiet()
    until(lambda:lab.sql("SELECT count(*) FROM messages WHERE body IS NOT NULL OR styles IS NOT NULL OR conference IS NOT NULL")==[(0,)])
    assert lab.sql("PRAGMA integrity_check")==[("ok",)]
def privacy_and_invalid(lab):
    fourth(lab);lab.start();a=lab.client(1);b=lab.client(2);c=lab.client(3);d=lab.client(4)
    for client in [a,b,c,d]:barrier(client)
    rules(c,[(1,4)]);c.close()
    assert a.message(2,77,options=STYLE+context([3,4]))==3;received(b,[3,4])
    assert a.message(3,77,options=context([2,4])+STYLE)==6
    assert a.message(4,77,options=context([2,3])+STYLE)==3;received(d,[2,3])
    assert lab.sql("SELECT count(*) FROM messages")==[(2,)]
    invalid=[context([1]),context([2]),context([3,3]),context([0]),context([0x1000000]),context(range(3,34)),b"\1"+words(0xffffffff),context([3])+context([4]),STYLE+STYLE,context([3])+b"\x04"]
    for value in invalid:assert a.message(2,78,options=value)==6
    assert a.message(999,79,options=context([3,4]))==6
    assert lab.sql("SELECT count(*) FROM messages")==[(2,)]
    # A context does not authorize delivery to its listed users; blocked copies stay absent.
    c=lab.client(3);barrier(c);c.quiet()
    assert a.message(2,77,flags=8|32,options=context([4]),ack=False) is None
    received(b,[4],styled=False);a.quiet();d.quiet();c.quiet()
def quota(lab):
    lab.start(extra="[queue]\nper_recipient_bytes=1024\n");a=lab.client(1)
    others=list(range(1000,1030))
    assert a.message(2,77,"x"*500,options=context(others))==3
    assert a.message(2,77,"x"*500,options=context(others))==4
    assert lab.sql("SELECT count(*),sum(payload_bytes) FROM messages")==[(1,653)]
    b=lab.client(2);received(b,others,text="x"*500,styled=False,queued=True);b.ping();b.quiet()
def crash(lab,stage):
    lab.start(fault=stage);a=lab.client(1)
    a.message(2,0xfeed,"awaria",options=context([3])+STYLE,ack=False)
    lab.server.process.wait(timeout=5);assert lab.server.process.returncode==86
    lab.stop();lab.start();b=lab.client(2)
    if stage=="before_accept_commit":b.quiet();assert lab.sql("SELECT count(*) FROM messages")==[(0,)]
    else:
        kind,body=b.receive();assert kind==10 and body[16:]==b"awaria\0"+context([3])+STYLE
        b.ping();b.quiet()
    assert lab.sql("PRAGMA integrity_check")==[("ok",)]
def baseline_migration(lab,server,admin):
    lab.start();a=lab.client(1);assert a.message(2,77,"stara baza",options=STYLE)==3
    assert lab.sql("SELECT max(version) FROM _sqlx_migrations")==[(6,)]
    before=lab.sql("SELECT payload_bytes FROM messages")[0][0]
    lab.stop(hard=True);lab.binary=server;lab.admin=admin;lab.start()
    assert lab.sql("SELECT max(version) FROM _sqlx_migrations")==[(10,)]
    assert lab.sql("SELECT conference,payload_bytes FROM messages")==[("[]",before+2)]
    b=lab.client(2);kind,body=b.receive();assert kind==10 and body[16:]==b"stara baza\0"+STYLE
    b.ping();a=lab.client(1);assert a.message(2,77,options=context([3])+STYLE)==3;received(b,[3])
    assert lab.sql("PRAGMA integrity_check")==[("ok",)]
def main():
    p=argparse.ArgumentParser();p.add_argument("--server",type=Path,required=True);p.add_argument("--admin",type=Path,required=True);p.add_argument("--faults",action="store_true");p.add_argument("--baseline-dir",type=Path);args=p.parse_args()
    cases=[("four participants: one copy each, ordered context/styles, offline restart",four_people),("per-target privacy, malformed contexts and no-ACK",privacy_and_invalid),("participant bound and context bytes charged to quota",quota)]
    if args.faults:
        for stage in ["before_accept_commit","after_accept_commit"]:cases.append(("conference crash "+stage,lambda lab,stage=stage:crash(lab,stage)))
    for name,test in cases:
        with tempfile.TemporaryDirectory(prefix="rgg-conference-") as root:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
            try:test(lab);print("PASS:",name,flush=True)
            finally:lab.stop()
    if args.baseline_dir:
        with tempfile.TemporaryDirectory(prefix="rgg-conference-upgrade-") as root:
            lab=Lab((args.baseline_dir/"rgg-server").resolve(),(args.baseline_dir/"rgg-admin").resolve(),Path(root))
            try:baseline_migration(lab,args.server.resolve(),args.admin.resolve());print("PASS: actual dev.8 schema-6 queue upgrades with authenticated delivery",flush=True)
            finally:lab.stop()
if __name__=="__main__":main()
