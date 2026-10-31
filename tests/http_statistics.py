# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Anonymous counts against real local HTTP/TCP, synthetic users only."""
import argparse,json,tempfile,time
from pathlib import Path
from gg6_sessions import Lab
from gg6_presence_contacts import barrier,status
from http_discovery import request
from http_registration import form,fields,submit,created

def counters(lab,registered,online):
    code,headers,body=request(lab,"/api/stats")
    assert code==200,(code,body)
    assert headers["cache-control"]=="no-store" and headers["x-content-type-options"]=="nosniff"
    value=json.loads(body)
    assert set(value)=={"registered","publicly_online","sampled_at"},value
    assert (value["registered"],value["publicly_online"])==(registered,online),value
    assert abs(value["sampled_at"]-time.time())<5
    return value

def privacy(lab):
    lab.start(statistics=True);counters(lab,3,0)
    loading=lab.client(1,ready=False);loading.ping();counters(lab,3,0)
    loading.send(0x12);barrier(loading);counters(lab,3,1)
    b=lab.client(2);barrier(b);status(b,0x16,"PrivateCanary",123);barrier(b);counters(lab,3,1)
    c=lab.client(3);barrier(c);status(c,0x8003);barrier(c);counters(lab,3,1)
    status(b,3);barrier(b);counters(lab,3,2)
    status(loading,0x8002);barrier(loading);counters(lab,3,1)
    status(b,0x15,"OfflineCanary",123);b.closed();counters(lab,3,0)
    status(c,2);barrier(c);counters(lab,3,1)
    replacement=lab.client(3);barrier(replacement);counters(lab,3,1)
    home=request(lab,"/");assert home[0]==200
    assert b'data-online="1"' in home[2] and b'data-registered="3"' in home[2]
    assert b'href="/register"' not in home[2] and b'PrivateCanary' not in home[2]
    assert request(lab,"/register")[0]==404 and request(lab,"/assets/registration.css")[0]==200
    lab.stop(hard=True);lab.start(statistics=True);counters(lab,3,0)
    lab.stop();lab.run("retire","--uin","2");lab.start(statistics=True);counters(lab,2,0)

def registration(lab):
    code=lab.run("invite")["codes"][0]
    lab.start(statistics=True,registration=True);counters(lab,3,0)
    token,cookie=form(lab);assert created(submit(lab,fields(token,code,42),cookie))==42
    counters(lab,4,0)
    home=request(lab,"/");assert home[0]==200 and b'href="/register"' in home[2]
    assert code.encode() not in home[2] and b'synthetic@example.invalid' not in home[2]
    lab.stop();lab.start(registration=True)
    assert request(lab,"/")[0]==404 and request(lab,"/api/stats")[0]==404
    assert request(lab,"/register")[0]==200

def bounds(lab):
    lab.start()
    assert request(lab,"/")[0]==404 and request(lab,"/api/stats")[0]==404
    lab.stop();lab.start(statistics=True)
    assert request(lab,"/api/stats",headers={"Host":"attacker.invalid"})[0]==403
    assert request(lab,"/api/stats",headers={"Sec-Fetch-Site":"cross-site"})[0]==403
    for _ in range(60):assert request(lab,"/api/stats")[0]==200
    assert request(lab,"/api/stats",headers={"X-Forwarded-For":"192.0.2.1"})[0]==429
    assert request(lab,"/")[0]==429 and request(lab,"/assets/registration.css")[0]==429
    assert request(lab,"/health/ready")[0]==200

def main():
    p=argparse.ArgumentParser();p.add_argument("--server",type=Path,required=True);p.add_argument("--admin",type=Path,required=True);args=p.parse_args()
    for name,test in [("snapshot privacy, readiness, replacement, restart and retirement",privacy),("registration integration and independent opt-ins",registration),("disabled endpoints, Host and shared source-IP budgets",bounds)]:
        with tempfile.TemporaryDirectory(prefix="rgg-statistics-") as root:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
            try:test(lab);print("PASS:",name,flush=True)
            finally:lab.stop()
if __name__=="__main__":main()
