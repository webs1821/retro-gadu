# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Browser patcher delivery/guards against real loopback HTTP, with synthetic accounts only."""
import argparse,json,subprocess,tempfile
from pathlib import Path
from gg6_sessions import Lab
from http_discovery import request
from http_downloads import fixture
ROOT=Path(__file__).resolve().parents[1]
CONFIG="\n[client_patcher]\ndefault_host='rgg.local'\n"
ASSETS={'/assets/client-patcher.js':'client-patcher.mjs','/assets/client-patcher-core.js':'client-patcher-core.mjs','/assets/client-patch-plan.json':'client-patch-plan.json'}

def delivery(lab):
    lab.start(statistics=True)
    for path in ['/patcher',*ASSETS]:assert request(lab,path)[0]==404
    assert b'href="/patcher"' not in request(lab,'/')[2]
    lab.stop();fixture(lab.root);lab.start(extra=CONFIG,statistics=True,registration=True,downloads='artifacts/manifest.toml')
    code,headers,body=request(lab,'/patcher');assert code==200
    assert b'value="rgg.local"' in body and b'type="file"' in body and b'download="gg.exe"' in body
    assert b'name="' not in body.split(b'<form',1)[1].split(b'</form>',1)[0] # no fallback file upload
    csp=headers['content-security-policy'];assert "script-src 'self'" in csp and "connect-src 'self'" in csp and "form-action 'none'" in csp
    assert 'unsafe-inline' not in csp and headers['cache-control']=='no-store' and headers['x-content-type-options']=='nosniff'
    assert request(lab,'/patcher',method='HEAD')[2]==b''
    for path,file in ASSETS.items():
        code,headers,body=request(lab,path);assert code==200 and body==(ROOT/'crates/rgg-http/src'/file).read_bytes()
        assert headers['content-type'].startswith('application/json' if path.endswith('.json') else 'text/javascript')
        assert headers['cache-control']=='no-store' and 'access-control-allow-origin' not in headers
        assert not request(lab,path,method='HEAD')[2]
    recipe=json.loads(request(lab,'/assets/client-patch-plan.json')[2]);assert recipe['client_size']==790528 and len(recipe['edits'])==24
    for path in ['/','/downloads']:
        code,headers,body=request(lab,path);assert code==200 and b'href="/patcher"' in body
        assert "script-src" not in headers['content-security-policy']
    assert b'fetch(' in request(lab,'/assets/client-patcher.js')[2]
    lab.client(1).ping()

def guards(lab):
    lab.start(extra=CONFIG)
    assert request(lab,'/')[0]==404 and request(lab,'/register')[0]==404 and request(lab,'/downloads')[0]==404
    for path in ['/patcher',*ASSETS]:
        assert request(lab,path,method='POST',body=b'NEVER_UPLOAD_THIS_FILE')[0]==405
        assert request(lab,path+'?file=anything')[0]==400
        assert request(lab,path,body=b'body')[0]==400
        assert request(lab,path,headers={'Host':'attacker.invalid'})[0]==403
        assert request(lab,path,headers={'Origin':'https://attacker.invalid'})[0]==403
        assert request(lab,path,headers={'Sec-Fetch-Site':'cross-site'})[0]==403
        assert request(lab,path,headers={'Origin':'http://'+lab.server.address})[0]==200
    lab.stop();lab.start(extra=CONFIG)
    for _ in range(60):assert request(lab,'/assets/client-patch-plan.json')[0]==200
    assert request(lab,'/patcher',headers={'X-Forwarded-For':'192.0.2.1'})[0]==429
    assert request(lab,'/health/ready')[0]==200;lab.client(1).ping()
    lab.stop()
    config=lab.root/'bad.toml'
    for host in ['', 'longhostname.invalid', 'a..b', '<script>', 'a:80']:
        config.write_text("data_dir='never-created'\nhttp_bind='127.0.0.1:0'\n[client_patcher]\ndefault_host="+json.dumps(host)+'\n')
        result=subprocess.run([str(lab.binary),'--config',str(config)],capture_output=True,timeout=5)
        assert result.returncode!=0 and not (lab.root/'never-created').exists()

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--server',type=Path,required=True);parser.add_argument('--admin',type=Path,required=True);args=parser.parse_args()
    for name,test in [('opt-in, exact embedded assets, CSP and navigation',delivery),('no upload, Host/Origin/body/query guards, shared budget and early config validation',guards)]:
        with tempfile.TemporaryDirectory(prefix='rgg-http-patcher-') as tmp:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(tmp))
            try:test(lab);print('PASS:',name,flush=True)
            finally:lab.stop()
if __name__=='__main__':main()
