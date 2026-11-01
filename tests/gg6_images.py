# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Independent libgadu-wire-reference image fixtures; synthetic accounts, real TCP/processes."""
import argparse,struct,tempfile,zlib
from pathlib import Path
from gg6_sessions import Lab,words,until,PASSWORD,oracle
from gg6_presence_contacts import barrier,rules
DATA=bytes(range(256))*20
def ident(data=DATA):return words(len(data),zlib.crc32(data))
def style(data=DATA):
    rich=b'\0\0\x88\xff\0\x7f\x09\x01'+ident(data)
    return b'\x02'+struct.pack('<H',len(rich))+rich
def offer(a,target=2,seq=77,data=DATA):assert a.message(target,seq,'\xa0',options=style(data))==3
def text(b,data=DATA):
    k,p=b.receive();assert k==10 and p[16:]==b'\xa0\0'+style(data),(k,p)
def control(c,target,flag,tail=b'',data=DATA,ack=True):return c.message(target,7,'',flags=4 if ack else 36,options=bytes([flag])+ident(data)+tail,ack=ack)
def recv(c,source,flag,tail=None,data=DATA):
    k,p=c.receive();assert k==10 and p[:4]==words(source) and p[12:17]==words(4)+b'\0',(k,p)
    assert p[17:26]==bytes([flag])+ident(data),(flag,p)
    if tail is not None:assert p[26:]==tail,(tail,p)
    return p[26:]
def download(a,b,data=DATA):
    assert control(b,1,4,data=data)==2;recv(a,2,4,b'',data)
    assert control(a,2,5,b'test.png\0'+data[:1800],data)==2;b.quiet()
    for chunk in [data[i:i+1800] for i in range(1800,len(data),1800)]:assert control(a,2,6,chunk,data)==2
    result=recv(b,1,5,data=data);name,chunk=result.split(b'\0',1);assert name==b'test.png';result=chunk
    while len(result)<len(data):result+=recv(b,1,6,data=data)
    assert result==data;b.quiet();a.quiet()
def online(lab):
    lab.start();a=lab.client(1);b=lab.client(2);c=lab.client(3)
    for client in [a,b,c]:barrier(client)
    offer(a);text(b);until(lambda:lab.sql('SELECT count(*) FROM messages WHERE body IS NOT NULL')==[(0,)])
    download(a,b)
    assert lab.sql('SELECT count(*) FROM image_offers')==[(1,)]
    assert lab.sql('SELECT count(*) FROM messages')==[(1,)] # controls never enter durable text queue
    assert control(c,1,4)==6;recv(c,1,5,b'');a.quiet()
    assert control(c,2,5,b'test.png\0'+DATA[:100])==6;b.quiet()
    assert control(b,1,4,ack=False) is None;recv(a,2,4,b'');b.quiet()
    assert control(a,2,5)==2;recv(b,1,5,b'')
def offline(lab):
    lab.start();a=lab.client(1);barrier(a);offer(a);assert lab.sql('SELECT count(*) FROM image_offers')==[(1,)]
    lab.stop(hard=True);lab.start();b=lab.client(2);text(b)
    assert control(b,1,4)==6;recv(b,1,5,b'')
    a=lab.client(1);a.send(2,words(0x8014));barrier(a);download(a,b) # invisible/friends-only still honors an explicit offer
    lab.stop(hard=True);lab.start();a=lab.client(1);b=lab.client(2)
    for c in [a,b]:barrier(c);c.quiet()
    download(a,b);assert lab.sql('SELECT count(*) FROM messages WHERE styles IS NOT NULL')==[(0,)]
def invalid_and_fences(lab):
    lab.start();a=lab.client(1);b=lab.client(2)
    for c in [a,b]:barrier(c)
    offer(a);text(b)
    assert control(b,1,4)==2;recv(a,2,4,b'')
    assert control(a,2,6,b'first is not continuation')==6;recv(b,1,5,b'')
    assert control(b,1,4)==2;recv(a,2,4,b'')
    assert control(a,2,5,b'test.png\0'+DATA[:1800])==2
    rules(b,[(1,4)])
    for chunk in [DATA[i:i+1800] for i in range(1800,len(DATA),1800)]:assert control(a,2,6,chunk)==2
    recv(b,1,5,b'');assert lab.sql('SELECT count(*) FROM image_offers')==[(0,)]
    rules(b,[]);assert control(b,1,4)==6;recv(b,1,5,b'')
    offer(a);text(b);assert control(b,1,4)==2;recv(a,2,4,b'')
    newer=lab.client(1);a.closed();barrier(newer);recv(b,1,5,b'')
    assert control(newer,2,5,b'test.png\0'+DATA[:1800])==6;b.quiet()
    assert control(b,1,4)==2;recv(newer,2,4,b'')
    broken=bytes([DATA[0]^1])+DATA[1:]
    assert control(newer,2,5,b'test.png\0'+broken[:1800])==2
    assert control(newer,2,6,broken[1800:3600])==2
    assert control(newer,2,6,broken[3600:])==6;recv(b,1,5,b'')
    # Malformed control mixed with a style is rejected, never silently persisted.
    assert newer.message(2,8,'',flags=8,options=b'\4'+ident())==6
    assert newer.message(2,8,'',flags=4,options=b'\4'+ident()+style())==6
    assert newer.message(2,8,'x',options=b'\4'+ident())==6
    assert lab.sql('PRAGMA integrity_check')==[('ok',)]
def capabilities(lab):
    lab.start();a=lab.client(1);b=lab.client();
    b.send(0x15,words(2,oracle(PASSWORD,b.seed),2,0x20)+bytes(13)+bytes([1,0xbe]));assert b.receive()==(3,b'');b.send(0x12)
    for c in [a,b]:barrier(c)
    offer(a);text(b);assert control(b,1,4)==6;recv(b,1,5,b'');a.quiet()
    # Only image capacity is disclosed, and only with visible presence.
    a.send(0xd,words(2)+b'\1');k,p=a.receive();assert k==15 and p[:5]==words(2)+b'\2';assert p[5:12]==bytes(7) and p[12:14]==bytes([1,0])
    b.send(2,words(0x14));barrier(b);k,p=a.receive();assert k==15 and p[4]==1 and p[5:14]==bytes(9)
    # A later login declaring no image support cannot inherit the earlier capability.
    a.send(0xe,words(2)+b'\1');barrier(a)
    new=lab.client();new.send(0x15,words(2,oracle(PASSWORD,new.seed),2,0x20)+bytes(15));assert new.receive()==(3,b'');new.send(0x12);b.closed();barrier(new)
    assert control(new,1,4)==6;recv(new,1,5,b'')
def maximum_and_conference(lab):
    from gg6_conferences import context
    lab.start();a=lab.client(1);b=lab.client(2);c=lab.client(3)
    for client in [a,b,c]:barrier(client)
    data=bytes(range(256))*1020 # exactly 255 KiB, the GG6 byte-sized capability ceiling
    assert a.message(2,77,'\xa0',options=context([3])+style(data))==3
    k,p=b.receive();assert k==10 and p[16:]==b'\xa0\0'+context([3])+style(data)
    assert control(c,1,4,data=data)==6;recv(c,1,5,b'',data);a.quiet()
    download(a,b,data);c.quiet()
def crash(lab,stage):
    lab.start(fault=stage);a=lab.client(1);a.message(2,0xfeed,'\xa0',options=style(),ack=False)
    lab.server.process.wait(timeout=5);assert lab.server.process.returncode==86;lab.stop();lab.start()
    expected=0 if stage=='before_accept_commit' else 1
    assert lab.sql('SELECT count(*) FROM image_offers')==[(expected,)]
    b=lab.client(2)
    if expected:text(b);a=lab.client(1);barrier(a);download(a,b)
    else:b.quiet();assert lab.sql('SELECT count(*) FROM messages')==[(0,)]
def migration(lab,server,admin):
    lab.start();a=lab.client(1);assert a.message(2,77,'schema seven')==3
    assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(7,)]
    lab.stop(hard=True);lab.binary=server;lab.admin=admin;lab.start();assert lab.sql('SELECT max(version) FROM _sqlx_migrations')==[(9,)]
    b=lab.client(2);k,p=b.receive();assert k==10 and p[16:]==b'schema seven\0';barrier(b)
    a=lab.client(1);barrier(a);offer(a);text(b);download(a,b)
def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);p.add_argument('--faults',action='store_true');p.add_argument('--baseline-dir',type=Path);args=p.parse_args()
    cases=[('online verified chunks, recipient authorization and no-ACK',online),('offline references, missing sender, restart and post-delivery retrieval',offline),('ordering CRC blocks malformed packets and session replacement',invalid_and_fences),('LOGIN60 capacity limits and hidden presence',capabilities),('255 KiB image and conference context grants no additional access',maximum_and_conference)]
    if args.faults:
        for stage in ['before_accept_commit','after_accept_commit']:cases.append(('atomic image offer crash '+stage,lambda lab,stage=stage:crash(lab,stage)))
    for name,test in cases:
        with tempfile.TemporaryDirectory(prefix='rgg-images-') as root:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
            try:test(lab);print('PASS:',name,flush=True)
            finally:lab.stop()
    if args.baseline_dir:
        with tempfile.TemporaryDirectory(prefix='rgg-images-upgrade-') as root:
            lab=Lab((args.baseline_dir/'rgg-server').resolve(),(args.baseline_dir/'rgg-admin').resolve(),Path(root))
            try:migration(lab,args.server.resolve(),args.admin.resolve());print('PASS: actual dev.9 queue migration and image transfer',flush=True)
            finally:lab.stop()
if __name__=='__main__':main()
