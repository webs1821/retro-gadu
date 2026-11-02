# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Conference fan-out and classic images with an external unmodified libgadu client."""
import argparse,hashlib,json,os,socket,struct,tempfile,zlib
from pathlib import Path
from gg6_sessions import Lab,until
from libgadu_client import Probe,WireRelay,sql

DATA=bytes(range(256))*20
CRC=zlib.crc32(DATA)
FORMAT=struct.pack('<HBHII',0,0x80,0x0109,len(DATA),CRC).hex()

def run_case(args,root,evidence,mode):
    lab=Lab(args.server,args.admin,root);relay=WireRelay();clients=[]
    def start():lab.start();relay.target=lab.address
    def connect(user):
        c=Probe(args.probe,relay.port,user,root);clients.append(c);c.take('login',uin=user,protocol_version=32)
        c.command('empty');c.ping();return c
    def stopped(*active,hard=False):
        lab.stop(hard=hard)
        for c in active:c.take('closed');assert c.process.wait(timeout=3)==0
    def offer(a):
        a.command('offer 2');seq=a.take('sent')['seq'];a.take('ack',recipient=2,status=3,seq=seq)
    def image_text(b,offline):
        m=b.take('message',sender=1)
        assert m['text']=='a0' and m['formats']==FORMAT and m['class']==(9 if offline else 8) and m['recipients']==[],m
    def missing(client):
        client.command('image-get 1');client.take('ack',recipient=1,status=6,seq=0)
        item=client.take('image_reply',sender=1)
        assert item['size']==0 and item['data']=='' and item['filename']=='',item
    def download(a,b):
        b.command('image-get 1');b.take('ack',recipient=1,status=2,seq=0)
        a.take('image_request',sender=2,size=len(DATA),crc32=CRC)
        a.command('image-send 2')
        for _ in range(3):a.take('ack',recipient=2,status=2,seq=0)
        item=b.take('image_reply',sender=1,size=len(DATA),crc32=CRC)
        assert item['data']==DATA.hex() and item['filename']==b'obrazek.bin'.hex()
        a.ping();b.ping()
    try:
        start();a=connect(1);b=connect(2)
        if mode=='conference':
            text='Konferencja: żółć';a.command('conference 0 '+text.encode('cp1250').hex())
            seq=a.take('sent')['seq']
            for recipient in [2,3]:a.take('ack',recipient=recipient,status=3,seq=seq)
            m=b.take('message',sender=1)
            assert m['text']==text.encode('cp1250').hex() and m['recipients']==[3] and m['class']==8
            assert sql(lab,'SELECT recipient,count(*) FROM messages GROUP BY recipient ORDER BY recipient')==[(2,1),(3,1)]
            assert sql(lab,"SELECT count(*) FROM deliveries WHERE state='Pending'")==[(1,)]
            stopped(a,b,hard=True);start();c=connect(3)
            m=c.take('message',sender=1)
            assert m['text']==text.encode('cp1250').hex() and m['recipients']==[2] and m['class']==9
            b=connect(2);b.ping();assert not any(x['event']=='message' for x in b.events)
            stopped(b,c)
            assert sql(lab,"SELECT state,count(*) FROM deliveries GROUP BY state")==[('TransportWrittenUnconfirmed',2)]
            evidence.update(client_fanout=True,one_copy_per_recipient=True,recipient_context_preserved=True,offline_after_crash=True)
        else:
            outsider=connect(3);offer(a);image_text(b,False);missing(outsider);download(a,b)
            b.close();until(lambda:sql(lab,'SELECT count(*) FROM account_sessions WHERE uin=2')==[(0,)])
            offer(a);assert sql(lab,"SELECT count(*) FROM deliveries WHERE state='Pending'")==[(1,)]
            stopped(a,outsider,hard=True);start();b=connect(2);image_text(b,True);missing(b)
            a=connect(1);a.command('status 20 '+b'Invisible'.hex());a.ping();download(a,b)
            stopped(a,b)
            counts={'messages':sql(lab,'SELECT count(*) FROM messages')[0][0],
                    'offers':sql(lab,'SELECT count(*) FROM image_offers')[0][0]}
            evidence['database_counts']=counts
            # Offers are scoped to message_id; repeating an image is a new authorized offer.
            assert counts=={'messages':2,'offers':2},counts
            evidence.update(image_bytes=len(DATA),crc32=CRC,exact_binary_data=True,chunks_from_sender=3,
                            unauthorized_recipient_refused=True,offline_offer_survives_restart=True,
                            missing_sender_refused=True,invisible_sender_can_reply=True,no_durable_image_cache=True)
        assert sql(lab,'PRAGMA integrity_check')==[('ok',)] and sql(lab,'PRAGMA foreign_key_check')==[]
        evidence['passed']=True
    finally:
        evidence.update(login_captures=relay.records,client_events=[c.trace for c in clients])
        for c in clients:c.close()
        lab.stop();relay.close()

def main():
    parser=argparse.ArgumentParser()
    for name in ['server','admin','probe','report']:parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--outer-network-namespace',required=True);args=parser.parse_args()
    namespace=os.readlink('/proc/self/ns/net');assert namespace!=args.outer_network_namespace and args.outer_network_namespace.startswith('net:[')
    assert {name for _,name in socket.if_nameindex()}=={'lo'} and not args.report.exists()
    report={'passed':False,'loopback_only':True,'network_namespace':namespace,'outer_network_namespace':args.outer_network_namespace,
            'binary_sha256':{n:hashlib.sha256(getattr(args,n).read_bytes()).hexdigest() for n in ['server','admin','probe']},
            'driver_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'cases':{},'native_gg_qualified':False}
    try:
        for mode in ['conference','images']:
            evidence={};report['cases'][mode]=evidence
            with tempfile.TemporaryDirectory(prefix='rgg-libgadu-'+mode+'-') as root:run_case(args,Path(root),evidence,mode)
            print('PASS: libgadu '+mode,flush=True)
        report['passed']=True
    except BaseException as error:report['error']=repr(error);raise
    finally:args.report.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')

if __name__=='__main__':main()
