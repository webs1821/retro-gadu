# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Generate only bounded offset/format facts for the browser recipe; no client binary copy."""
import argparse,json
from pathlib import Path
from build_client import CLIENT_POSITION
from client_format import Invalid,SolidInstaller
from client_patch import CLIENT_SIZE,CLIENT_SHA256,INSTALLER_SHA256,HOSTS,patch_client,sha

def plan(original):
    _,edits=patch_client(original,'127.0.0.1')
    result=[]
    for edit in sorted(edits,key=lambda item:item.start):
        item=dict(offset=edit.start,length=len(edit.before))
        if edit.reason=='RT_STRING':item['kind']='rt_string'
        elif edit.reason.endswith(' endpoint'):
            item.update(kind='cstring',width=1 if edit.reason.startswith('ascii') else 2)
        else:item.update(kind='fixed',before=edit.before.hex(),after=edit.after.hex())
        result.append(item)
    return dict(schema=1,license='AGPL-3.0-or-later',authors=['webs1821'],
                client_version='6.1.0.158',client_size=CLIENT_SIZE,client_sha256=CLIENT_SHA256,
                domains=list(HOSTS),edits=result)

def original_client(path):
    with path.open('rb') as source:data=source.read(2*1024*1024+1)
    if len(data)>2*1024*1024:raise Invalid('installer exceeds the bounded source size')
    if sha(data)!=INSTALLER_SHA256:raise Invalid('only the pinned original installer is accepted')
    stream=SolidInstaller.parse(data).stream
    return stream[CLIENT_POSITION:CLIENT_POSITION+CLIENT_SIZE]

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--installer',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--check',action='store_true');args=p.parse_args()
    generated=plan(original_client(args.installer))
    if args.check:assert json.loads(args.output.read_text(encoding='utf-8'))==generated
    else:
        with args.output.open('x',encoding='utf-8',newline='\n') as f:json.dump(generated,f,indent=2);f.write('\n')
    print('PASS: bounded browser recipe, %d ranges; no executable stored'%len(generated['edits']))
if __name__=='__main__':main()
