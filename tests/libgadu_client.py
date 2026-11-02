# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""External libgadu 1.7.0 API client; isolated network, synthetic data, unchanged wire."""
import argparse, contextlib, hashlib, json, os, select, socket, sqlite3, struct
import subprocess, tempfile, threading, time
from pathlib import Path
from gg6_sessions import Lab, until


class WireRelay:
    """Bounded transparent relay; record login metadata, never its challenge response."""
    def __init__(self):
        self.listener=socket.socket(); self.listener.bind(('127.0.0.1',0)); self.listener.listen(4)
        self.port=self.listener.getsockname()[1]; self.target=None; self.records=[]
        self.stop=threading.Event(); self.failure=None
        self.thread=threading.Thread(target=self.run); self.thread.start()
    def run(self):
        pairs={}; inputs={}
        def close(s):
            peer=pairs.pop(s,None)
            if peer is not None: pairs.pop(peer,None); inputs.pop(peer,None); peer.close()
            inputs.pop(s,None); s.close()
        try:
            while not self.stop.is_set():
                readable,_,_=select.select([self.listener,*pairs],[],[],.05)
                for source in readable:
                    if source is self.listener:
                        a,address=self.listener.accept()
                        assert address[0]=='127.0.0.1' and len(pairs)<8 and len(self.records)<12
                        b=socket.create_connection(self.target,timeout=1)
                        a.settimeout(1); b.settimeout(1); pairs[a]=b; pairs[b]=a
                        record={'packets':[],'bytes':0}; self.records.append(record); inputs[a]=[bytearray(),record]
                    elif source in pairs:
                        try:
                            raw=source.recv(8192)
                            if not raw: close(source); continue
                            if source in inputs:
                                pending,record=inputs[source]; pending.extend(raw); record['bytes']+=len(raw)
                                assert record['bytes']<65536 and len(pending)<16384
                                while len(pending)>=8:
                                    kind,size=struct.unpack('<II',pending[:8]); assert size<=8192
                                    if len(pending)<size+8: break
                                    if not record['packets']:
                                        assert kind==0x15 and size in (31,35), (kind,size)
                                        record.update(login_type=kind,login_length=size,uin=int.from_bytes(pending[8:12],'little'),protocol_version=int.from_bytes(pending[20:24],'little'))
                                        if size>31:
                                            record['description_hex']=bytes(pending[39:8+size]).hex()
                                            assert record['description_hex']=='aff3b3e6'
                                        assert record['protocol_version']==0x20
                                    record['packets'].append(kind); assert len(record['packets'])<=64
                                    if kind==2:record.setdefault('status_packets',[]).append(bytes(pending[8:8+size]).hex())
                                    del pending[:8+size]
                            pairs[source].sendall(raw)
                        except OSError: close(source)
        except BaseException as error: self.failure=repr(error)
        finally:
            for s in list(pairs): s.close()
    def close(self):
        self.stop.set(); self.thread.join(timeout=3); self.listener.close()
        assert not self.thread.is_alive() and self.failure is None,self.failure


class Probe:
    def __init__(self,binary,port,user,root,wrong=False,described=False):
        self.log=(root/f'client-{user}-{time.monotonic_ns()}.log').open('wb')
        self.process=subprocess.Popen([str(binary),str(port),str(user),*(['wrong-password'] if wrong else ['description'] if described else [])],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.log,bufsize=0)
        self.pending=bytearray(); self.events=[]; self.trace=[]
    def pump(self,seconds):
        if select.select([self.process.stdout],[],[],seconds)[0]:
            raw=os.read(self.process.stdout.fileno(),4096)
            if not raw: return False
            self.pending.extend(raw); assert len(self.pending)<16384
            while b'\n' in self.pending:
                line,_,tail=self.pending.partition(b'\n'); self.pending=bytearray(tail)
                event=json.loads(line); self.events.append(event); self.trace.append(event); assert len(self.trace)<256
        return True
    def take(self,event,**values):
        deadline=time.monotonic()+5
        while True:
            for index,item in enumerate(self.events):
                if item.get('event')==event and all(item.get(k)==v for k,v in values.items()):return self.events.pop(index)
            assert time.monotonic()<deadline, (event,values,self.events,self.process.poll())
            assert self.pump(.1), (event,values,self.events,self.process.poll())
    def command(self,text):
        self.process.stdin.write((text+'\n').encode()); self.take('command',name=text.split()[0])
    def ping(self): self.command('ping'); self.take('pong')
    def send(self,target,text):
        self.command(f'send {target} {text.encode("cp1250").hex()}')
        seq=self.take('sent')['seq']; self.take('ack',recipient=target,status=3,seq=seq)
    def received(self,sender,text,offline):
        item=self.take('message',sender=sender)
        assert item['text']==text.encode('cp1250').hex() and item['time']>0,item
        assert item['class']==(9 if offline else 8),item
    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:self.process.kill(); self.process.wait(timeout=3)
        self.process.stdout.close(); self.log.close()


def sql(lab,query):
    with contextlib.closing(sqlite3.connect(lab.data/'rgg.sqlite')) as db:return db.execute(query).fetchall()


def exercise(server,admin,probe,root,evidence):
    lab=Lab(server,admin,root); relay=WireRelay(); clients=[]
    def connect(user,wrong=False,described=False):
        c=Probe(probe,relay.port,user,root,wrong,described); clients.append(c)
        c.take('login_rejected' if wrong else 'login',**({} if wrong else {'uin':user,'protocol_version':32}))
        return c
    try:
        lab.start(); relay.target=lab.address
        bad=connect(1,True); assert bad.process.wait(timeout=3)==0
        sender=connect(1); sender.command('notify 2'); sender.take('notify',uin=2,status=1); sender.ping()
        texts=['Żółć sprzed restartu','druga wiadomość','trzecia: ąćęłńóśźż']
        for text in texts:sender.send(2,text)
        assert sql(lab,"SELECT count(*) FROM deliveries WHERE state='Pending'")==[(3,)]
        lab.stop(hard=True); sender.take('closed'); assert sender.process.wait(timeout=3)==0
        lab.start(); relay.target=lab.address
        receiver=connect(2); receiver.pump(.15)
        assert not receiver.events and sql(lab,"SELECT count(*) FROM deliveries WHERE state='Pending'")==[(3,)]
        receiver.command('empty')
        for text in texts: receiver.received(1,text,offline=True)
        receiver.ping(); until(lambda:sql(lab,"SELECT count(*) FROM deliveries WHERE state='Pending'")==[(0,)])
        sender=connect(1,described=True); sender.command('empty'); sender.ping()
        receiver.command('notify 1'); receiver.take('notify',uin=1,status=4,description='aff3b3e6')
        description='Żółty most'; sender.command('status 4 '+description.encode('cp1250').hex())
        receiver.take('status',uin=1,status=4,description=description.encode('cp1250').hex())
        sender.command('status 20 '+b'Hidden'.hex()); receiver.take('status',uin=1,status=1,description='')
        sender.send(2,'wiadomość online'); receiver.received(1,'wiadomość online',offline=False)
        receiver.send(1,'odpowiedź'); sender.received(2,'odpowiedź',offline=False)
        sender.ping(); receiver.ping(); lab.stop()
        for c in [sender,receiver]: c.take('closed'); assert c.process.wait(timeout=3)==0
        assert sql(lab,'PRAGMA integrity_check')==[('ok',)] and sql(lab,'PRAGMA foreign_key_check')==[]
        assert sql(lab,'SELECT count(*) FROM account_sessions')==[(0,)]
        assert sql(lab,"SELECT state,count(*) FROM deliveries GROUP BY state")==[('TransportWrittenUnconfirmed',5)]
        assert sql(lab,'SELECT count(*) FROM messages WHERE body IS NOT NULL')==[(0,)]
        assert len(relay.records)==4 and all(x['login_type']==0x15 and x['protocol_version']==32 for x in relay.records)
        return {'login_captures':relay.records,'client_events':[c.trace for c in clients],
                'offline_messages_after_crash':3,'online_messages':2,'invisibility':True,'description_cp1250':True,
                'readiness_gate':True,'incorrect_password_rejected':True,'clean_shutdown':True,'login_description_cp1250':True,
                'database_integrity':True,'library_sources_modified':False,'native_gg_qualified':False}
    finally:
        evidence.update(login_captures=relay.records,client_events=[c.trace for c in clients])
        for c in clients:c.close()
        lab.stop(); relay.close()


def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--server',type=Path,required=True); parser.add_argument('--admin',type=Path,required=True)
    parser.add_argument('--probe',type=Path,required=True); parser.add_argument('--report',type=Path,required=True)
    parser.add_argument('--outer-network-namespace',required=True); args=parser.parse_args()
    assert not args.report.exists()
    namespace=os.readlink('/proc/self/ns/net')
    assert args.outer_network_namespace.startswith('net:[') and namespace!=args.outer_network_namespace,'requires an isolated network namespace'
    interfaces=socket.if_nameindex()
    assert {name for _,name in interfaces}=={'lo'},'only loopback may exist'
    result={'passed':False,'network_namespace':namespace,'outer_network_namespace':args.outer_network_namespace,'interfaces':interfaces,'loopback_only':True,
            'binary_sha256':{k:hashlib.sha256(getattr(args,k).read_bytes()).hexdigest() for k in ['server','admin','probe']},
            'driver_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    try:
        with tempfile.TemporaryDirectory(prefix='rgg-libgadu-client-') as temporary:
            result.update(exercise(args.server.resolve(),args.admin.resolve(),args.probe.resolve(),Path(temporary),result))
        result['passed']=True
    except BaseException as error:
        result['error']=repr(error); raise
    finally:
        args.report.write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print('PASS: unmodified libgadu 1.7.0 LOGIN60, CP1250, presence, online/offline, crash/restart and readiness',flush=True)

if __name__=='__main__':main()
