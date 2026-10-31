# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Local HTTP discovery/legacy refusal; no DNS overrides or official GG requests."""
import argparse, http.client, socket, tempfile
from pathlib import Path
from gg6_sessions import Lab, Client

def request(lab,path,method="GET",body=None,headers=None):
    connection=http.client.HTTPConnection(lab.server.address,timeout=3)
    try:
        connection.request(method,path,body=body,headers=headers or {})
        reply=connection.getresponse();return reply.status,dict(reply.getheaders()),reply.read()
    finally:connection.close()

def main():
    p=argparse.ArgumentParser();p.add_argument("--server",type=Path,required=True);p.add_argument("--admin",type=Path,required=True);args=p.parse_args()
    with tempfile.TemporaryDirectory(prefix="rgg-discovery-") as root:
        lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
        code=lab.run("invite")["codes"][0]
        before=lab.sql("SELECT count(*),sum(used) FROM invitations")
        try:
            lab.start()
            assert request(lab,"/appsvc/appmsg3.asp")[2]==b"0 0 notoperating notoperating\r\n"
            lab.stop();lab.start(discovery=True)
            expected=("0 0 %s:%d %s\r\n"%(lab.address[0],lab.address[1],lab.address[0])).encode()
            for path in ["/appsvc/appmsg3.asp","/appsvc/appmsg4.asp"]:
                status,headers,body=request(lab,path+"?fmnumber=1&pwd=SYNTHETIC_HTTP_CANARY",headers={"Host":"untrusted.example","X-Forwarded-Host":"other.example","X-Forwarded-For":"203.0.113.7"})
                assert status==200 and body==expected
                assert headers["cache-control"]=="no-store" and headers["connection"]=="close"
            address=expected.split()[2].decode().split(":")
            client=Client((address[0],int(address[1])));lab.clients.append(client);assert client.login(1)==(3,b"");client.ping()
            # HTTP/1.0 with no Host, as old GG clients may send. Connection closes after reply.
            host,port=lab.server.address.split(":")
            with socket.create_connection((host,int(port)),timeout=3) as raw:
                raw.sendall(b"GET /appsvc/appmsg3.asp HTTP/1.0\r\n\r\n")
                data=b""
                while True:
                    part=raw.recv(4096)
                    if not part:break
                    data+=part
                assert data.split(b"\r\n\r\n",1)[1]==expected
            for path in ["/appsvc/fmregister3.asp","/fmregister.php","/fmsendpwd.php"]:
                status,_,body=request(lab,path,method="POST",body="fmnumber=1&pwd=OtherPass9!&invite="+code,headers={"Content-Type":"application/x-www-form-urlencoded"})
                assert status==403 and b"success" not in body
            assert lab.sql("SELECT count(*),sum(used) FROM invitations")==before
            assert lab.sql("SELECT count(*) FROM accounts")==[(3,)]
            assert b"SYNTHETIC_HTTP_CANARY" not in lab.server.log.read_bytes()
            client.close();again=lab.client(1);again.ping() # password remained unchanged
            assert request(lab,"/appsvc/appmsg_ver8.asp")[0]==404
            print("PASS: disabled/enabled discovery, actual loopback listener, hostile Host ignored, HTTP/1.0 and GG login",flush=True)
            print("PASS: legacy account operations refused without invitation/account mutations or secret logging",flush=True)
        finally:lab.stop()
if __name__=="__main__":main()
