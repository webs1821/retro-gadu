# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Cross-language full-byte comparison; optional supplied original is read only as data."""
import argparse,base64,json,subprocess,sys,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'tools'))
from client_package import synthetic_pe
from client_patch import HOSTS,Edit,domain_edits,apply_edits,patch_client,sha
from client_plan import original_client,plan

def encoded(data):return base64.b64encode(data).decode()

def main():
    p=argparse.ArgumentParser();p.add_argument('--node',default='node');p.add_argument('--installer',type=Path);args=p.parse_args()
    vectors=[]
    original,_,marker_offset=synthetic_pe()
    marker=original[marker_offset:marker_offset+4]
    for host in ['127.0.0.1','rgg.local','abcdefghijklmnop']:
        edits=domain_edits(original,host)+[Edit(marker_offset,marker,bytes([0x41])*len(marker),'test marker')]
        entries=[]
        for edit in sorted(edits,key=lambda item:item.start):
            item=dict(offset=edit.start,length=len(edit.before))
            if edit.reason=='RT_STRING':item['kind']='rt_string'
            elif edit.reason.endswith(' endpoint'):item.update(kind='cstring',width=1 if edit.reason.startswith('ascii') else 2)
            else:item.update(kind='fixed',before=edit.before.hex(),after=edit.after.hex())
            entries.append(item)
        recipe=dict(schema=1,client_size=len(original),client_sha256=sha(original),domains=list(HOSTS),edits=entries)
        vectors.append(dict(input=encoded(original),host=host,plan=recipe,expected=encoded(apply_edits(original,edits))))
    if args.installer:
        original=original_client(args.installer);recipe=plan(original)
        assert recipe==json.loads((ROOT/'crates/rgg-http/src/client-patch-plan.json').read_text(encoding='utf-8'))
        for host in ['127.0.0.1','rgg.local','abcdefghijklmnop']:
            result,_=patch_client(original,host)
            vectors.append(dict(input=encoded(original),host=host,plan=recipe,expected=encoded(result)))
    with tempfile.TemporaryDirectory(prefix='rgg-browser-core-') as tmp:
        data=Path(tmp)/'vectors.json';data.write_text(json.dumps(vectors),encoding='utf-8')
        subprocess.run([args.node,str(ROOT/'tests/client_browser.mjs'),str(data)],check=True,timeout=40)
    print('PASS: original executable was not executed or retained',flush=True)
if __name__=='__main__':main()
