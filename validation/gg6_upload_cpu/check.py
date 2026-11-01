# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 webs1821
"""Execute only the hash-pinned uploader in a CPU emulator; no Windows loader/API/network."""
import sys,struct,json,hashlib,argparse
from pathlib import Path
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--original',type=Path,required=True)
parser.add_argument('--patched',type=Path,required=True)
parser.add_argument('--report',type=Path,required=True)
parser.add_argument('--dependency-dir',type=Path)
ARGS=parser.parse_args()
if ARGS.report.exists():raise SystemExit('report must be new')
if ARGS.dependency_dir:sys.path.insert(0,str(ARGS.dependency_dir.resolve()))
import unicorn
assert unicorn.__version__=='2.1.4','use the pinned validation tool version'
from unicorn import Uc,UC_ARCH_X86,UC_MODE_32,UC_HOOK_CODE,UC_PROT_READ,UC_PROT_WRITE,UC_PROT_EXEC
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_EIP,UC_X86_REG_EAX,UC_X86_REG_EBX,UC_X86_REG_ECX,UC_X86_REG_EDX,UC_X86_REG_ESI,UC_X86_REG_EDI,UC_X86_REG_EBP
START=0x4249ef;END=0x424a8c;CAVE=0x479400
STRLEN=0x44fcc0;COPY=0x44fd50;SEND=0x42459a
DATA=0x1000000;OBJECT=0x2000000;STACK=0x3000000;RETURN=0x5000000

def run(image,data,patch=False,null=False):
    assert len(data)<=65535 and b'\0' not in data
    u=Uc(UC_ARCH_X86,UC_MODE_32)
    for address in [0x424000,0x44f000,0x479000,RETURN]:u.mem_map(address,4096,UC_PROT_READ|UC_PROT_EXEC)
    u.mem_write(START,image[START-0x400000:END-0x400000])
    if patch:u.mem_write(CAVE,image[CAVE-0x400000:CAVE-0x400000+128])
    u.mem_map(DATA,65536,UC_PROT_READ);u.mem_write(DATA,data+b'\0')
    u.mem_map(OBJECT,4096,UC_PROT_READ|UC_PROT_WRITE);u.mem_write(OBJECT+0x9c,struct.pack('<I',0x13572468))
    u.mem_map(STACK,65536,UC_PROT_READ|UC_PROT_WRITE)
    stack=STACK+65536-64;u.mem_write(stack,struct.pack('<II',RETURN,0 if null else DATA))
    initial={UC_X86_REG_EBX:0x11335577,UC_X86_REG_ESI:0x22446688,UC_X86_REG_EDI:0x33557799,UC_X86_REG_EBP:0x446688aa}
    for reg,value in initial.items():u.reg_write(reg,value)
    u.reg_write(UC_X86_REG_ECX,OBJECT);u.reg_write(UC_X86_REG_ESP,stack);u.reg_write(UC_X86_REG_EAX,0x55667788)
    frames=[];calls={STRLEN:0,COPY:0,SEND:0}
    def word(address):return struct.unpack('<I',u.mem_read(address,4))[0]
    def finish(value,arguments=0):
        sp=u.reg_read(UC_X86_REG_ESP);destination=word(sp)
        u.reg_write(UC_X86_REG_EAX,value);u.reg_write(UC_X86_REG_ECX,0xdeadbeef);u.reg_write(UC_X86_REG_EDX,0xabcdef01)
        u.reg_write(UC_X86_REG_ESP,sp+4+arguments);u.reg_write(UC_X86_REG_EIP,destination)
    def hook(uc,address,size,_):
        if address in calls:
            calls[address]+=1;sp=u.reg_read(UC_X86_REG_ESP)
            if address==STRLEN:
                ptr=word(sp+4);assert DATA<=ptr<=DATA+len(data)
                finish(bytes(u.mem_read(ptr,DATA+len(data)+1-ptr)).index(0))
            elif address==COPY:
                destination,source,length=[word(sp+i) for i in (4,8,12)]
                assert 0<=length<=2047 and STACK<=destination<STACK+65536 and DATA<=source<=DATA+len(data)
                if length:u.mem_write(destination,bytes(u.mem_read(source,length)))
                finish(destination)
            else:
                kind,ptr,length=[word(sp+i) for i in (4,8,12)]
                assert u.reg_read(UC_X86_REG_ECX)==OBJECT and kind==0x16 and 1<=length<=2048
                assert STACK<=ptr and ptr+length<=STACK+65536
                frames.append(bytes(u.mem_read(ptr,length)));assert len(frames)<=26
                finish(1,12)
        else:assert START<=address<END or patch and CAVE<=address<CAVE+128,(hex(address),size)
    u.hook_add(UC_HOOK_CODE,hook)
    u.emu_start(START,RETURN,timeout=1_000_000,count=100_000)
    assert u.reg_read(UC_X86_REG_EIP)==RETURN and u.reg_read(UC_X86_REG_ESP)==stack+8
    assert all(u.reg_read(reg)==value for reg,value in initial.items())
    assert word(OBJECT+0x9c)==len(frames) and bytes(u.mem_read(DATA,len(data)+1))==data+b'\0'
    expected_object=bytearray(4096);struct.pack_into('<I',expected_object,0x9c,len(frames))
    assert bytes(u.mem_read(OBJECT,4096))==expected_object and bytes(u.mem_read(stack+8,56))==bytes(56)
    payload=b''.join(f[1:] for f in frames)
    assert payload==(b'' if null else data[:50000])
    return dict(frames=frames,eax=u.reg_read(UC_X86_REG_EAX),length=len(data),null=null,types=[f[0] for f in frames],chunks=[len(f)-1 for f in frames],packet_counter=word(OBJECT+0x9c),payload_sha256=hashlib.sha256(payload).hexdigest(),calls={hex(k):v for k,v in calls.items()})


def payload(size):
    pattern='ąłŻ'.encode('cp1250')
    return (pattern*((size+len(pattern)-1)//len(pattern)))[:size]

def load_image(path):
    with path.open('rb') as source:data=source.read(790529)
    assert len(data)==790528,'unexpected executable size'
    return data

original=load_image(ARGS.original);patched=load_image(ARGS.patched)
assert hashlib.sha256(original).hexdigest()=='74b78af318c52f86dff8a7ee27a7a0b5ea545167aebb594a8a559ec97f2e3a57'
checks=[];transcripts=[]
sizes=sorted({0,1,49999,50000,50001,65535}|{2047*n+d for n in range(1,25) for d in [-1,0,1]})
for size in sizes:
    data=payload(size);before=run(original,data);after=run(patched,data,patch=True)
    old_frames=before.pop('frames');new_frames=after.pop('frames')
    final=min(size,50000);extra=bool(final and final%2047==0)
    assert new_frames==old_frames+([b'\1'] if extra else [])
    assert before['eax']==after['eax']==0
    assert after['types']==[0]+[1]*(len(new_frames)-1)
    checks.append({'input_length':size,'original':before,'patched':after})
    if size in [0,2047,2048,4094,4095,49128,50000,50001]:
        transcripts.append({'input_length':size,'payload_sha256':after['payload_sha256'],'frames_hex':[frame.hex() for frame in new_frames]})
a=run(original,b'',null=True);b=run(patched,b'',patch=True,null=True)
assert a==b and a['frames']==[] and a['eax']==0x55667788
report={'format':1,'passed':True,'unicorn_version':unicorn.__version__,
        'original_sha256':hashlib.sha256(original).hexdigest(),'patched_sha256':hashlib.sha256(patched).hexdigest(),
        'driver_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'checks':checks,'null_unchanged':True,'transcripts':transcripts,
        'native_application_executed':False,'windows_api_called':False,'network_used':False}
with ARGS.report.open('x',encoding='utf-8',newline='\n') as output:json.dump(report,output,indent=2);output.write('\n')
print(json.dumps({'passed':True,'cases':len(checks)+1,'transcripts':len(transcripts),'native_application_executed':False}))
