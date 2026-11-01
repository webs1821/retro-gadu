# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Private operational snapshots on real loopback HTTP/GG, disposable synthetic data."""
import argparse,json,re,socket,tempfile,time
from pathlib import Path
from types import SimpleNamespace
from gg6_sessions import Lab,until
from gg6_presence_contacts import barrier,status
from http_discovery import request

CANARY="PrivateMessageCanary"
def endpoint(lab):
    match=re.search(r"metrics_endpoint=(127\.0\.0\.1:\d+)",lab.server.log.read_text())
    assert match,"metrics listener missing"
    return SimpleNamespace(server=SimpleNamespace(address=match.group(1)))

def metrics(lab):
    code,headers,body=request(endpoint(lab),"/metrics")
    assert code==200,(code,body)
    assert headers["content-type"]=="text/plain; version=0.0.4; charset=utf-8"
    assert headers["cache-control"]=="no-store" and headers["x-content-type-options"]=="nosniff"
    assert "access-control-allow-origin" not in headers
    assert body.endswith(b"\n") and b"\r" not in body and len(body)<4096
    for secret in [CANARY.encode(),b"synthetic@example.invalid",b"LocalTest9!",b"PrivateStatusCanary",b"uin=",b"sender=",b"recipient="]:
        assert secret not in body,secret
    types={};values={}
    for line in body.decode().splitlines():
        if line.startswith("# TYPE "):
            _,_,name,kind=line.split();assert name not in types and kind=="gauge";types[name]=kind
        elif not line.startswith("#"):
            match=re.fullmatch(r'(rgg_[a-z_]+(?:\{[a-z_]+="[A-Za-z0-9_.-]+"\})?) ([0-9]+(?:\.[0-9]+)?)',line)
            assert match,line
            key,value=match.groups();assert key not in values and key.split("{")[0] in types
            values[key]=float(value)
    assert len(types)==12 and len(values)==19,(types,values)
    assert values['rgg_database_schema_version']==11
    assert abs(values['rgg_snapshot_timestamp_seconds']-time.time())<3
    return values

def value(lab,key,expected):
    until(lambda:metrics(lab)[key]==expected)

def flow(lab):
    lab.start();assert request(lab,"/metrics")[0]==404
    assert "metrics_endpoint=" not in lab.server.log.read_text();lab.stop()
    lab.start(metrics=True,statistics=True)
    assert request(lab,"/metrics")[0]==404
    assert request(endpoint(lab),"/api/stats")[0]==404
    initial=metrics(lab);assert initial['rgg_accounts{state="active"}']==3
    a=lab.client(1);barrier(a);status(a,0x16,"PrivateStatusCanary");barrier(a)
    value(lab,'rgg_sessions{phase="ready"}',1)
    public=json.loads(request(lab,"/api/stats")[2]);assert public['publicly_online']==0
    assert a.message(2,77,CANARY)==3
    value(lab,'rgg_delivery_records{state="pending"}',1)
    assert metrics(lab)['rgg_queued_payload_bytes']>0
    lab.stop(hard=True);lab.start(metrics=True,statistics=True)
    sample=metrics(lab);assert sample['rgg_sessions{phase="ready"}']==0
    assert sample['rgg_delivery_records{state="pending"}']==1
    b=lab.client(2);kind,body=b.receive();assert kind==10 and body[16:]==CANARY.encode()+b"\0"
    value(lab,'rgg_delivery_records{state="written_unconfirmed"}',1)
    sample=metrics(lab);assert sample['rgg_queued_payload_bytes']==0 and sample['rgg_oldest_queue_age_seconds']==0
    assert sample['rgg_delivery_records{state="pending"}']==0
    assert CANARY not in lab.server.log.read_text()
    lab.stop();lab.run("retire","--uin","3");lab.start(metrics=True)
    sample=metrics(lab);assert sample['rgg_accounts{state="active"}']==2 and sample['rgg_retired_numbers']==1

def guards(lab):
    lab.start(metrics=True);private=endpoint(lab)
    for headers in [{"Host":"attacker.invalid"},{"Origin":"https://attacker.invalid"},{"Sec-Fetch-Site":"cross-site"},{"Sec-Fetch-Site":"same-site"},{"Host":"attacker.invalid","X-Forwarded-Host":private.server.address,"X-Forwarded-For":"127.0.0.1"}]:
        assert request(private,"/metrics",headers=headers)[0]==403
    assert request(private,"/metrics?"+CANARY)[0]==400
    assert request(private,"/metrics",method="POST")[0]==405
    assert request(private,"/metrics",method="GET",body=b"abc")[0]==400
    assert request(private,"/metrics",method="HEAD")[0:1]==(200,)
    assert request(private,"/metrics",method="HEAD")[2]==b""
    assert request(private,"/metrics",headers={"Origin":"http://"+private.server.address,"Sec-Fetch-Site":"same-origin"})[0]==200
    host,port=private.server.address.split(":")
    with socket.create_connection((host,int(port)),timeout=3) as conn:
        conn.sendall(("GET /metrics HTTP/1.1\r\nHost: "+private.server.address+"\r\nOrigin: http://"+private.server.address+"\r\nOrigin: http://"+private.server.address+"\r\nConnection: close\r\n\r\n").encode())
        assert conn.recv(4096).startswith(b"HTTP/1.1 403")
    stalled=[]
    try:
        for _ in range(4):
            conn=socket.create_connection((host,int(port)),timeout=3);conn.sendall(b"G");stalled.append(conn)
        assert request(lab,"/health/ready")[0]==200
        c=lab.client(1);c.ping()
        start=time.monotonic();lab.stop();assert time.monotonic()-start<9
    finally:
        for conn in stalled:conn.close()

def main():
    p=argparse.ArgumentParser();p.add_argument("--server",type=Path,required=True);p.add_argument("--admin",type=Path,required=True);args=p.parse_args()
    for name,test in [("private gauges, invisible users, offline queue, restart and retirement",flow),("dedicated endpoint, browser guards, bounded sockets and clean stop",guards)]:
        with tempfile.TemporaryDirectory(prefix="rgg-metrics-") as root:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
            try:test(lab);print("PASS:",name,flush=True)
            finally:lab.stop()
if __name__=="__main__":main()
