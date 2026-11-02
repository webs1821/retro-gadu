# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Public directory and contact archives through unmodified external libgadu APIs."""
import argparse, hashlib, json, os, socket, tempfile
from pathlib import Path
from gg6_sessions import Lab
from libgadu_client import Probe, WireRelay, sql


def directory(client,command,seq,kind,count,text=None):
    client.command(f'{command} {seq}'+(' '+text.encode('cp1250').hex() if text is not None else ''))
    reply=client.take('directory',seq=seq,type=kind)
    assert reply['count']==count and reply['next']==0,reply
    return reply['rows']


def contacts(client,text=None):
    if text is not None:
        client.command('put 0 '+text.encode('cp1250').hex() if text else 'clear-list')
        client.take('userlist',type=0,text='')
    client.command('get');result=client.take('userlist',type=6)['text']
    if text is not None:assert result==text.encode('cp1250').hex()
    return result


def exercise(args,root,evidence):
    lab=Lab(args.server,args.admin,root);relay=WireRelay();clients=[]
    def connect(user):
        client=Probe(args.probe,relay.port,user,root);clients.append(client)
        client.take('login',uin=user,protocol_version=32);client.command('empty');client.ping();return client
    try:
        lab.start();relay.target=lab.address;owner=connect(1);observer=connect(2)
        assert contacts(owner)=='' and contacts(observer)==''
        directory(observer,'age',101,5,0)
        directory(owner,'profile',102,1,0,'Żółwik')
        own=directory(owner,'read',103,3,1)[0]
        assert own['nickname']=='Żółwik'.encode('cp1250').hex() and own['city']=='Łódź'.encode('cp1250').hex()
        rows=directory(observer,'search',104,5,1,'zolwik')
        assert rows[0]['FmNumber']==b'1'.hex() and rows[0]['birthyear']==b'1987'.hex()
        assert rows[0]['nickname']==own['nickname'] and rows[0]['city']==own['city']
        assert directory(observer,'age',105,5,1)[0]['FmNumber']==b'1'.hex()
        assert directory(observer,'active',106,5,1)[0]['FmStatus']==b'2'.hex()
        owner.command('status 20 '+b'Private'.hex());owner.ping()
        directory(observer,'active',107,5,0)
        assert directory(observer,'search',108,5,1,'Żółwik')[0]['FmStatus']==b'1'.hex()
        texts=['Jan;Kowalski;Janek;123\r\n','ą'*2046,'ę'*2048]
        for text in texts:
            contacts(owner,text);assert contacts(observer)==''
        lab.stop()
        for c in [owner,observer]:c.take('closed');assert c.process.wait(timeout=3)==0
        lab.start();relay.target=lab.address;owner=connect(1);observer=connect(2)
        assert contacts(owner)==('ę'*2048).encode('cp1250').hex() and contacts(observer)==''
        assert directory(owner,'read',109,3,1)[0]['nickname']==own['nickname']
        assert directory(observer,'age',110,5,1)[0]['FmNumber']==b'1'.hex()
        contacts(owner,'');directory(owner,'unpublish',111,1,0);directory(observer,'age',112,5,0)
        owner.ping();observer.ping();lab.stop()
        for c in [owner,observer]:c.take('closed');assert c.process.wait(timeout=3)==0
        assert sql(lab,'SELECT count(*) FROM public_profiles')==[(0,)]
        assert sql(lab,'PRAGMA integrity_check')==[('ok',)] and sql(lab,'PRAGMA foreign_key_check')==[]
        evidence.update(passed=True,profile_write_read_search=True,polish_search=True,age_single_row_no_phantom=True,
                        invisible_active_only=True,profile_persistence=True,profile_unpublished=True,
                        contact_lengths=[0,*[len(text.encode('cp1250')) for text in texts]],contact_isolation=True,contact_persistence=True,contact_clear=True,
                        exact_full_2047_unqualified=True,native_gg_qualified=False)
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
    evidence={'passed':False,'network_namespace':namespace,'outer_network_namespace':args.outer_network_namespace,'loopback_only':True,
              'binary_sha256':{n:hashlib.sha256(getattr(args,n).read_bytes()).hexdigest() for n in ['server','admin','probe']},
              'driver_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    try:
        with tempfile.TemporaryDirectory(prefix='rgg-libgadu-directory-') as temporary:exercise(args,Path(temporary),evidence)
    except BaseException as error:evidence['error']=repr(error);raise
    finally:args.report.write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8')
    print('PASS: libgadu directory write/read/search, Polish filters, age, privacy, contacts and restart',flush=True)

if __name__=='__main__':main()
