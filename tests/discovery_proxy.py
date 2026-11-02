# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Advertised GG endpoint through a bounded loopback-only TCP relay, never public networking."""
import argparse,contextlib,ipaddress,json,select,socket,sqlite3,subprocess,tempfile,threading
from pathlib import Path
from gg6_sessions import Lab,Client,until
from http_discovery import request

class Relay:
    def __enter__(self):
        self.listener=socket.socket();self.listener.bind(('127.0.0.2',0));self.listener.listen(4)
        self.listener.setblocking(False);self.address=self.listener.getsockname()
        self.stop=threading.Event();self.upstream=None;self.accepted=0;self.forwarded=0;self.failure=None
        self.thread=threading.Thread(target=self.run,name='rgg-test-loopback-relay');self.thread.start()
        return self
    def target(self,address):
        assert ipaddress.ip_address(address[0]).is_loopback and address[1]>0
        self.upstream=address
    def run(self):
        pairs={}
        def close(sock):
            other=pairs.pop(sock,None)
            if other is not None:pairs.pop(other,None);other.close()
            sock.close()
        try:
            while not self.stop.is_set():
                ready,_,_=select.select([self.listener,*pairs],[],[],.1)
                for current in ready:
                    if current is self.listener:
                        try:incoming,address=self.listener.accept()
                        except BlockingIOError:continue
                        if self.upstream is None or len(pairs)>=16 or not ipaddress.ip_address(address[0]).is_loopback:
                            incoming.close();continue
                        try:outgoing=socket.create_connection(self.upstream,timeout=1)
                        except OSError:incoming.close();continue
                        incoming.settimeout(1);outgoing.settimeout(1)
                        pairs[incoming]=outgoing;pairs[outgoing]=incoming;self.accepted+=1
                    elif current in pairs:
                        try:
                            data=current.recv(65536)
                            if not data:close(current);continue
                            pairs[current].sendall(data);self.forwarded+=len(data)
                        except OSError:close(current)
        except Exception as error:self.failure=repr(error)
        finally:
            for sock in list(pairs):sock.close()
    def __exit__(self,*_):
        self.stop.set();self.thread.join(timeout=3);self.listener.close()
        assert not self.thread.is_alive(),'test relay failed to stop'
        assert self.failure is None,self.failure

def sql(lab,query):
    with contextlib.closing(sqlite3.connect(lab.data/'rgg.sqlite')) as db:return db.execute(query).fetchall()

def client_at(lab,address,user):
    client=Client(address);lab.clients.append(client);assert client.login(user)==(3,b'');client.ping();return client

def proxy_flow(lab):
    with Relay() as relay:
        advertised='%s:%d'%relay.address
        lab.start(discovery=True,gg_advertise=advertised);relay.target(lab.address)
        assert relay.address!=lab.address and lab.address[0]=='127.0.0.1'
        expected=('0 0 '+advertised+' '+relay.address[0]+'\r\n').encode()
        for path in ('/appsvc/appmsg3.asp','/appsvc/appmsg4.asp'):
            code,headers,body=request(lab,path+'?fmnumber=1&server=203.0.113.7:80',headers={
                'Host':'attacker.invalid','X-Forwarded-Host':'attacker.invalid',
                'X-Forwarded-For':'203.0.113.7','X-Forwarded-Proto':'https'})
            assert code==200 and body==expected
            assert headers['cache-control']=='no-store' and headers['connection']=='close'
        host,port=lab.server.address.split(':')
        with socket.create_connection((host,int(port)),timeout=3) as connection:
            connection.sendall(b'GET /appsvc/appmsg3.asp HTTP/1.0\r\n\r\n');reply=b''
            while True:
                data=connection.recv(4096)
                if not data:break
                reply+=data;assert len(reply)<4096
            assert reply.split(b'\r\n\r\n',1)[1]==expected
        discovered=expected.split()[2].decode().split(':');address=(discovered[0],int(discovered[1]))
        sender=client_at(lab,address,1);receiver=client_at(lab,address,2)
        assert sender.message(2,101,'ViaLocalRelay')==3
        assert receiver.receive()[1][16:]==b'ViaLocalRelay\0'
        receiver.close();until(lambda:sql(lab,'SELECT count(*) FROM account_sessions WHERE uin=2')==[(0,)])
        assert sender.message(2,102,'QueuedViaRelay')==3
        assert sql(lab,"SELECT count(*) FROM deliveries WHERE state='Pending'")==[(1,)]
        old_backend=lab.address;lab.stop()
        lab.start(discovery=True,gg_advertise=advertised);relay.target(lab.address)
        assert request(lab,'/appsvc/appmsg3.asp')[2]==expected
        receiver=Client(address);lab.clients.append(receiver);assert receiver.login(2)==(3,b'')
        kind,body=receiver.receive();assert kind==10 and body[16:]==b'QueuedViaRelay\0'
        assert int.from_bytes(body[12:16],'little')&1
        receiver.ping();again=client_at(lab,address,1);again.ping()
        lab.stop()
        assert sql(lab,'PRAGMA integrity_check')==[('ok',)] and sql(lab,'PRAGMA foreign_key_check')==[]
        assert relay.accepted>=4 and relay.forwarded>0
        print('PASS: explicit loopback relay, hostile headers, HTTP/1.0, GG login, online/offline and restart',flush=True)
        return {'relay_connections':relay.accepted,'forwarded_bytes':relay.forwarded,'backend_changed':lab.address!=old_backend}

def invalid_configurations(lab):
    base="data_dir='{data}'\nhttp_bind='127.0.0.1:0'\ngg_bind='127.0.0.1:0'\nlegacy_discovery=true\n"
    cases=[]
    for value in ('0.0.0.0:8074','0.1.2.3:8074','224.0.0.1:8074','240.0.0.1:8074',
                  '255.255.255.255:8074','127.0.0.1:0','127.0.0.1:65536','[::1]:8074','ConfigCanary.invalid:8074'):
        cases.append(base+'gg_advertise='+json.dumps(value)+'\n')
    valid="gg_advertise='198.51.100.7:18074'\n"
    cases += [base.replace('legacy_discovery=true','legacy_discovery=false')+valid,
              base.replace("gg_bind='127.0.0.1:0'\n",'')+valid,
              base.replace("gg_bind='127.0.0.1:0'","gg_bind='[::1]:0'")+valid]
    for field in ('http_bind','gg_bind'):
        cases.append(base.replace(field+"='127.0.0.1:0'",field+"='0.0.0.0:0'")+valid)
    cases.append(base+valid+"metrics_bind='0.0.0.0:0'\n")
    for index,source in enumerate(cases):
        name='refused-'+str(index);config=lab.root/(name+'.toml');config.write_text(source.format(data=name))
        result=subprocess.run([str(lab.binary),'--config',str(config)],capture_output=True,timeout=5)
        assert result.returncode!=0 and not (lab.root/name).exists()
        assert b'ConfigCanary' not in result.stderr
    print('PASS:',len(cases),'invalid configurations rejected before creating a database',flush=True)
    return len(cases)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--server',type=Path,required=True);parser.add_argument('--admin',type=Path,required=True);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='rgg-advertised-') as root:
        lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
        try:
            refused=invalid_configurations(lab);proxy=proxy_flow(lab)
            print('ADVERTISED_ENDPOINT_REPORT='+json.dumps({'invalid_configs':refused,**proxy,'all_loopback':True,'native_client_qualified':False}),flush=True)
        finally:lab.stop()

if __name__=='__main__':main()
