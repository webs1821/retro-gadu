# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Real GG6 event accounting, histogram invariants, limits and restart boundaries."""
import argparse,socket,tempfile
from pathlib import Path
from gg6_sessions import Lab,words,until
from gg6_presence_contacts import barrier
from http_metrics import metrics,value

def operation(lab,kind,result,expected):value(lab,'rgg_operations_total{operation="'+kind+'",result="'+result+'"}',expected)
def ended(lab,reason,expected):value(lab,'rgg_gg_connection_ends_total{reason="'+reason+'"}',expected)

def logins(lab):
    lab.start(metrics=True)
    value(lab,'rgg_gg_connections',0)
    bad=lab.client();value(lab,'rgg_gg_connections',1)
    assert bad.login(1,password=b'WrongTest9!')==(9,b'');bad.closed();operation(lab,'login','rejected',1)
    wrong_kind=lab.client();wrong_kind.send(8);assert wrong_kind.receive()==(9,b'');wrong_kind.closed()
    oversized=lab.client();oversized.socket.sendall(words(0x15,1<<20));assert oversized.receive()==(9,b'');oversized.closed()
    partial=lab.client();partial.socket.sendall(words(0x15,30)+b'x');partial.socket.shutdown(socket.SHUT_WR);assert partial.receive()==(9,b'');partial.closed()
    operation(lab,'login','invalid',3)
    stalled=lab.client();assert stalled.receive()==(9,b'');stalled.closed();operation(lab,'login','timeout',1)
    a=lab.client(1);barrier(a);b=lab.client(1);barrier(b);a.closed();ended(lab,'replaced',1)
    operation(lab,'login','success',2);value(lab,'rgg_gg_connections',1)
    b.send(0x991);b.closed();ended(lab,'protocol',1);value(lab,'rgg_gg_connections',0)
    c=lab.client(2);barrier(c);c.close();ended(lab,'peer_closed',1)
    initial=lab.client(3,ready=False);initial.closed();ended(lab,'timeout',1)
    assert metrics(lab)['rgg_operation_duration_seconds_count{operation="login"}']==9

def messages(lab):
    lab.start(metrics=True,extra='\n[queue]\nper_recipient_messages=1\n')
    a=lab.client(1);barrier(a)
    assert a.message(2,11,'RuntimePrivateCanary')==3
    assert a.message(2,12,'quota')==4
    assert a.message(999,13,'rejected')==6
    assert a.message(2,14,'bad',options=b'\xff')==6
    for result in ['success','quota','rejected','invalid']:operation(lab,'accept',result,1)
    assert metrics(lab)['rgg_operation_duration_seconds_count{operation="delivery"}']==0
    before=metrics(lab)
    lab.stop(hard=True);lab.start(metrics=True)
    sample=metrics(lab)
    assert sample['rgg_operation_duration_seconds_count{operation="accept"}']==0 and sample['rgg_gg_admissions_total{result="accepted"}']==0
    assert sample['rgg_delivery_records{state="pending"}']==1
    b=lab.client(2);kind,body=b.receive();assert kind==10 and body[16:]==b'RuntimePrivateCanary\0'
    operation(lab,'delivery','success',1);assert metrics(lab)['rgg_operation_duration_seconds_sum{operation="delivery"}']>0
    b.close();value(lab,'rgg_gg_connections',0)
    assert before['rgg_operation_duration_seconds_count{operation="accept"}']==4

def admission(lab):
    lab.start(metrics=True,extra='max_connections=2\nper_ip=1\n')
    a=lab.client(1);barrier(a)
    with socket.create_connection(lab.address,timeout=3) as rejected:assert rejected.recv(1)==b''
    value(lab,'rgg_gg_admissions_total{result="ip_limit"}',1)
    with socket.create_connection(lab.address,timeout=3,source_address=('127.0.0.2',0)) as second:
        assert second.recv(12)
        value(lab,'rgg_gg_connections',2)
        with socket.create_connection(lab.address,timeout=3,source_address=('127.0.0.3',0)) as rejected:assert rejected.recv(1)==b''
        value(lab,'rgg_gg_admissions_total{result="global_limit"}',1)
    a.close();value(lab,'rgg_gg_connections',0)
    lab.stop();lab.start(metrics=True,extra='max_connections=128\nper_ip=128\n')
    for _ in range(100):
        with socket.create_connection(lab.address,timeout=3) as conn:conn.recv(12)
    assert metrics(lab)['rgg_gg_admissions_total{result="rate"}']>0
    value(lab,'rgg_gg_connections',0)

def blocked_delivery(lab):
    lab.start(metrics=True)
    sender=lab.client(1);receiver=lab.client(2,small=True);barrier(sender);barrier(receiver)
    for sequence in range(150):assert sender.message(2,sequence,'x'*1800)==3
    until(lambda:metrics(lab)['rgg_operations_total{operation="delivery",result="timeout"}']>=1,seconds=6)
    value(lab,'rgg_gg_connections',1);sender.ping()
    assert metrics(lab)['rgg_gg_connection_ends_total{reason="timeout"}']>=1
    assert lab.sql("SELECT count(*) FROM deliveries WHERE state='Pending'")[0][0]>0
    until(lambda:lab.sql("SELECT count(*) FROM deliveries WHERE state='Leased'")==[(0,)])
    assert lab.sql("SELECT count(*) FROM delivery_attempts WHERE outcome='Interrupted'")[0][0]>0
    receiver.close()

def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);args=p.parse_args()
    for name,test in [('login outcomes, malformed frames, timeout, replacement and socket release',logins),('accept results, cumulative histograms, offline replay and volatile reset',messages),('per-IP/global/rate admission counters and closed connection gauges',admission),('blocked TCP delivery is timed out, counted and released to Pending',blocked_delivery)]:
        with tempfile.TemporaryDirectory(prefix='rgg-runtime-metrics-') as root:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
            try:test(lab);print('PASS:',name,flush=True)
            finally:lab.stop()
if __name__=='__main__':main()
