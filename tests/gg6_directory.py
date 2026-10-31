# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Independent GG6 directory wire scenarios. Never connects to production/native GG."""
import argparse, struct, tempfile
from pathlib import Path
from gg6_sessions import Lab
from gg6_presence_contacts import take, rules, status, barrier

def pairs(data):
    if not data:return {}
    assert data.endswith(b"\0")
    parts=data[:-1].split(b"\0");assert len(parts)%2==0
    out={}
    for k,v in zip(parts[::2],parts[1::2]):
        k=k.decode("ascii").lower();assert k and k not in out;out[k]=v.decode("cp1250")
    return out

def query(c,kind=3,fields=None,seq=None,error=False):
    if seq is None:seq=0 if kind in (1,2) else 123
    body=bytes([kind])+struct.pack("<I",seq)
    for k,v in (fields or {}).items():body+=k.encode("ascii")+b"\0"+str(v).encode("cp1250")+b"\0"
    c.send(0x14,body);reply=take(c,0xe)
    assert reply[0]==({1:2,2:4,3:6} if error else {1:1,2:3,3:5})[kind],reply
    assert struct.unpack_from("<I",reply,1)[0]==seq
    if error or kind==1:assert len(reply)==5;return None
    if kind==2:return pairs(reply[5:])
    chunks=reply[5:].split(b"\0\0");rows=[];next_start=None
    for i,chunk in enumerate(chunks):
        record=pairs(chunk+(b"\0" if i<len(chunks)-1 else b""))
        if "nextstart" in record:assert len(record)==1;next_start=int(record["nextstart"])
        else:assert record.get("fmnumber");rows.append(record)
    assert next_start is not None
    return rows,next_start

def profile(lab):
    lab.start();a=lab.client(1);b=lab.client(2)
    assert query(b)==([],0)
    empty=query(a,2,seq=0xdeadbeef);assert empty["firstname"]=="" and empty["birthyear"]=="0"
    query(a,1,{"firstname":"Jan","birthyear":"1987"})
    rows,_=query(b,fields={"birthyear":"1980 1990"});assert len(rows)==1 and rows[0]["fmnumber"]=="1" and "city" not in rows[0]
    query(a,1,{"firstname":"Łucja","lastname":"Kowalska","nickname":"Żółwik","city":"Łódź","familyname":"Nowak","familycity":"Gdańsk","birthyear":"1984","gender":"2"})
    own=query(a,2);assert own["gender"]=="2" and own["familyname"]=="Nowak"
    rows,_=query(b,fields={"firstname":"lucja","lastname":"NOWAK","city":"gdansk","gender":"1","birthyear":"1984"})
    assert len(rows)==1 and set(rows[0])=={"fmnumber","firstname","nickname","city","birthyear","fmstatus"}
    assert query(b,fields={"gender":"2"})==([],0)
    query(a,1,{"nickname":"Nowy"});own=query(a,2);assert own["nickname"]=="Nowy" and own["firstname"]=="" and own["gender"]=="0"
    query(a,1,{"FmNumber":"2","nickname":"foreign"},error=True)
    query(b,2,{"FmNumber":"1"},error=True)
    query(a,1,{"birthyear":"2106"},error=True)
    assert query(a,2)["nickname"]=="Nowy"
    a.close();b.close();lab.stop();lab.start();a=lab.client(1);b=lab.client(2)
    assert query(a,2)["nickname"]=="Nowy"
    query(a,1,{"gender":"2"});assert query(b)==([],0)

def privacy(lab):
    lab.start();a=lab.client(1);b=lab.client(2);c=lab.client(3)
    query(a,1,{"nickname":"visible"})
    assert query(b,fields={"ActiveOnly":"1"})[0][0]["fmstatus"]=="2"
    status(a,0x16,"secret");barrier(a)
    assert query(b,fields={"ActiveOnly":"1"})==([],0)
    assert query(b,fields={"FmNumber":"1"})[0][0]["fmstatus"]=="1"
    rules(a,[(2,3)]);status(a,0x8004,"friends");barrier(a)
    assert query(b,fields={"ActiveOnly":"1"})[0][0]["fmstatus"]=="4"
    assert query(c,fields={"ActiveOnly":"1"})==([],0)
    a.send(0xd,struct.pack("<IB",2,4));barrier(a)
    assert query(b,fields={"FmNumber":"1"})[0][0]["fmstatus"]=="1"
    a.send(0xe,struct.pack("<IB",2,4));barrier(a)
    assert query(b,fields={"ActiveOnly":"1"})[0][0]["fmstatus"]=="4"
    rules(b,[(1,4)])
    assert query(b,fields={"ActiveOnly":"1"})==([],0)

def limits(lab):
    lab.start();a=lab.client(1,ready=False);a.send(0x14,b"\x02\0\0\0\0");a.closed()
    a=lab.client(1)
    query(a,3,seq=0,error=True);query(a,1,{"nickname":"x"*61},error=True)
    a.send(0x14,b"\x01\0\0\0\0nickname\0x\0NICKNAME\0y\0");assert take(a,0xe)[0]==2
    for _ in range(20):query(a,2)
    query(a,2,error=True);a.close();a=lab.client(1);query(a,2,error=True)
    a.close();lab.stop();lab.start();a=lab.client(1);query(a,2,error=True)
    assert lab.sql("SELECT count(*) FROM public_profiles")==[(0,)]
    assert lab.sql("PRAGMA integrity_check")==[("ok",)]

def main():
    p=argparse.ArgumentParser();p.add_argument("--server",type=Path,required=True);p.add_argument("--admin",type=Path,required=True);args=p.parse_args()
    for name,case in [("opt-in profiles, native reply forms, Polish filters and persistence",profile),("directory presence follows viewer privacy",privacy),("directory readiness, malformed fields and persistent rate limit",limits)]:
        with tempfile.TemporaryDirectory(prefix="rgg-directory-") as root:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
            try:case(lab);print("PASS:",name,flush=True)
            finally:lab.stop()
    print("PASS: synthetic directory suite; native GG6 qualification remains open",flush=True)
if __name__=="__main__":main()
