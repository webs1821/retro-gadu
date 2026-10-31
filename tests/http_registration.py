# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Synthetic registration over local HTTP, followed by real GG6 protocol login."""
import argparse, concurrent.futures, http.client, re, socket, tempfile, time, urllib.parse
from pathlib import Path
from gg6_sessions import Lab, Client
from http_discovery import request

PASSWORD="WebOnly9!"
def form(lab):
    status,headers,body=request(lab,"/register")
    assert status==200,(status,body)
    assert headers["cache-control"]=="no-store" and headers["referrer-policy"]=="no-referrer"
    assert "frame-ancestors 'none'" in headers["content-security-policy"]
    assert "HttpOnly" in headers["set-cookie"] and "SameSite=Strict" in headers["set-cookie"]
    csrf=re.search(rb'name="csrf" value="([0-9a-f]{64})"',body).group(1).decode()
    cookie=headers["set-cookie"].split(";",1)[0]
    return csrf,cookie
def fields(csrf,code,uin=""):
    return dict(requested_uin=str(uin),invitation=code,password=PASSWORD,email="synthetic@example.invalid",csrf=csrf)
def submit(lab,values,cookie,headers=None):
    base={"Origin":"http://"+lab.server.address,"Content-Type":"application/x-www-form-urlencoded","Cookie":cookie}
    if headers:
        for k,v in headers.items():
            if v is None:base.pop(k,None)
            else:base[k]=v
    return request(lab,"/register",method="POST",body=urllib.parse.urlencode(values),headers=base)
def created(response):
    status,headers,body=response
    assert status==201,(status,body)
    assert "Max-Age=0" in headers["set-cookie"]
    assert PASSWORD.encode() not in body
    return int(re.search(rb'data-uin="([0-9]+)"',body).group(1))
def used(lab):return lab.sql("SELECT sum(used) FROM invitations")[0][0]
def login(lab,user):
    client=Client(lab.address);lab.clients.append(client);assert client.login(user,password=PASSWORD.encode())==(3,b"");client.ping();return client

def normal(lab):
    code=lab.run("invite")["codes"][0];lab.start(registration=True,discovery=True)
    token,cookie=form(lab);assert created(submit(lab,fields(token,code,42),cookie))==42
    before=used(lab);token,cookie=form(lab);assert submit(lab,fields(token,code,42),cookie)[0]==409;assert used(lab)==before
    token,cookie=form(lab);first=created(submit(lab,fields(token,code.upper()),cookie));assert first==112233
    token,cookie=form(lab);second=created(submit(lab,fields(token,code),cookie));assert second==112234
    token,cookie=form(lab);assert submit(lab,fields(token,code),cookie)[0]==400
    assert used(lab)==6 and lab.sql("SELECT count(*) FROM accounts")==[(6,)]
    assert lab.sql("SELECT count(*) FROM public_profiles")==[(0,)]
    a=login(lab,42);b=login(lab,first);assert a.message(first,1,"hello web") == 3
    packet,body=b.receive();assert packet==10 and body[16:]==b"hello web\0"
    assert PASSWORD.encode() not in lab.server.log.read_bytes() and code.encode() not in lab.server.log.read_bytes()
    assert request(lab,"/assets/registration.css")[0]==200
    lab.stop();lab.start(registration=True);login(lab,second)

def validation(lab):
    code=lab.run("invite")["codes"][0];lab.start();assert request(lab,"/register")[0]==404
    lab.stop();lab.start(registration=True)
    assert request(lab,"/register",headers={"Host":"untrusted.example"})[0]==403
    csrf,cookie=form(lab);values=fields(csrf,code)
    for bad in [{"Origin":None},{"Origin":"https://evil.example"},{"Host":"evil.example"},{"Sec-Fetch-Site":"cross-site"},{"Cookie":None},{"Cookie":cookie+"; "+cookie}]:
        assert submit(lab,values,cookie,bad)[0]==403
    assert submit(lab,{**values,"csrf":"00"*32},cookie)[0]==403
    assert submit(lab,{**values,"extra":"not-allowed"},cookie)[0]==400
    assert submit(lab,{**values,"password":"SYNTHETIC_CANARY"*600},cookie)[0]==413
    assert used(lab)==3
    for edits in [{"requested_uin":"0"},{"requested_uin":"16777216"},{"password":"Zażółć123"},{"password":"short"},{"email":"invalid"},{"invitation":"00"*16}]:
        token,new_cookie=form(lab);status,_,body=submit(lab,{**fields(token,code),**edits},new_cookie)
        assert status==400 and PASSWORD.encode() not in body
    assert used(lab)==3
    result=submit(lab,values,cookie);user=created(result);assert user==112233
    assert submit(lab,values,cookie)[0]==403
    assert used(lab)==4 and lab.sql("SELECT count(*) FROM accounts")==[(4,)]
    assert b"SYNTHETIC_CANARY" not in lab.server.log.read_bytes()

def concurrent_registrations(lab):
    code=lab.run("invite")["codes"][0];lab.start(registration=True)
    prepared=[form(lab) for _ in range(12)]
    def register(item):
        token,cookie=item;return submit(lab,fields(token,code),cookie)
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:results=list(pool.map(register,prepared))
    successful=[created(response) for response in results if response[0]==201]
    assert len(successful)==3 and len(set(successful))==3,[r[0] for r in results]
    assert all(r[0] in (201,400,429) for r in results)
    assert used(lab)==6 and lab.sql("SELECT count(*) FROM accounts")==[(6,)]
    for user in successful:login(lab,user)
    assert lab.sql("PRAGMA integrity_check")==[("ok",)]

def rate_and_deadline(lab):
    code=lab.run("invite")["codes"][0];lab.start(registration=True)
    token,cookie=form(lab)
    host,port=lab.server.address.split(":")
    with socket.create_connection((host,int(port)),timeout=8) as raw:
        raw.sendall((f"POST /register HTTP/1.1\r\nHost: {lab.server.address}\r\nOrigin: http://{lab.server.address}\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 1000\r\nCookie: {cookie}\r\n\r\ncsrf=").encode())
        start=time.monotonic();reply=http.client.HTTPResponse(raw);reply.begin();assert reply.status==408;reply.read();assert time.monotonic()-start<7
    values=fields(token,"00"*16)
    for n in range(19):assert submit(lab,values,cookie,{"X-Forwarded-For":"203.0.113."+str(n)})[0] in (400,403)
    assert submit(lab,values,cookie)[0]==429
    assert used(lab)==3 and lab.sql("SELECT count(*) FROM accounts")==[(3,)]
    assert request(lab,"/health/ready")[0]==200

def main():
    p=argparse.ArgumentParser();p.add_argument("--server",type=Path,required=True);p.add_argument("--admin",type=Path,required=True);args=p.parse_args()
    for name,case in [("manual/automatic UIN, invite use, login, messaging and persistence",normal),("Host/Origin/CSRF, replay, malformed inputs and bounded body",validation),("twelve concurrent registrations share a three-use invitation",concurrent_registrations),("request deadline and source-IP budget ignore forwarded headers",rate_and_deadline)]:
        with tempfile.TemporaryDirectory(prefix="rgg-web-") as root:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
            try:case(lab);print("PASS:",name,flush=True)
            finally:lab.stop()
if __name__=="__main__":main()
