# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Local immutable downloads; every artifact here is synthetic and not executable software."""
import argparse,hashlib,json,subprocess,tempfile,http.client,socket,time
from pathlib import Path
from gg6_sessions import Lab
from http_discovery import request
CONTENT=b'MZ-SYNTHETIC-FIXTURE-NOT-A-REAL-CLIENT\0'+bytes(range(256))*8
def fixture(root,content=CONTENT,title='Klient testowy <script>alert(1)</script>'):
    folder=root/'artifacts';folder.mkdir(exist_ok=True)
    (folder/'client.exe').write_bytes(content)
    manifest='schema=1\n[[artifacts]]\nid="client"\nkind="client"\ntitle='+json.dumps(title)+'\nfile="client.exe"\nsize='+str(len(content))+'\nsha256="'+hashlib.sha256(content).hexdigest()+'"\nsource_url="https://example.invalid/source"\nlicense="Wyłącznie sztuczna próbka do testów"\ninstructions="Zamknij GG. Zrób kopię gg.exe w folderze instalacji, a następnie podmień ten plik. To nie jest instalator."\n'
    (folder/'manifest.toml').write_text(manifest,encoding='utf-8');return folder,manifest
def integrity(lab):
    folder,manifest=fixture(lab.root)
    lab.start();assert request(lab,'/downloads')[0]==404 and request(lab,'/downloads/client')[0]==404;lab.stop()
    lab.start(downloads='artifacts/manifest.toml',statistics=True,registration=True)
    code,headers,page=request(lab,'/downloads');assert code==200
    assert b'<script>' not in page and b'&lt;script&gt;' in page and b'client.exe' in page
    assert 'podmień'.encode() in page and 'nie jest instalator'.encode() in page
    assert b'href="/downloads"' in request(lab,'/')[2]
    code,headers,body=request(lab,'/downloads/client');assert code==200 and body==CONTENT
    assert headers['content-disposition']=='attachment; filename="client.exe"' and headers['content-type']=='application/octet-stream'
    assert int(headers['content-length'])==len(CONTENT) and headers['cache-control']=='no-store' and headers['x-content-type-options']=='nosniff'
    code,headers,body=request(lab,'/downloads/client',method='HEAD');assert code==200 and not body and int(headers['content-length'])==len(CONTENT)
    doc=json.loads(request(lab,'/api/downloads')[2]);assert doc['schema']==1 and doc['artifacts'][0]['sha256']==hashlib.sha256(CONTENT).hexdigest()
    assert str(folder).encode() not in request(lab,'/api/downloads')[2]
    (folder/'client.exe').write_bytes(b'CHANGED_AFTER_VALIDATION')
    assert request(lab,'/downloads/client')[2]==CONTENT # verified snapshot survives external replacement
    assert request(lab,'/downloads/client',headers={'Range':'bytes=0-2'})[0]==416
    for path in ['/downloads/nope','/downloads/%2e%2e','/downloads/%2e%2e%2fkeys.toml','/downloads/client.exe']:
        code,_,body=request(lab,path);assert code in (400,404) and b'MZ-SYNTHETIC' not in body
    assert request(lab,'/downloads/client?path=../../keys.toml')[2]==CONTENT
    assert request(lab,'/downloads/client',headers={'Host':'attacker.invalid'})[0]==403
    assert request(lab,'/downloads',headers={'Sec-Fetch-Site':'cross-site'})[0]==403
    lab.client(1).ping();assert request(lab,'/register')[0]==200
    lab.stop()
    config=lab.root/'bad-start.toml';config.write_text("data_dir='never-created'\nhttp_bind='127.0.0.1:0'\ndownloads_manifest='artifacts/manifest.toml'\n")
    result=subprocess.run([str(lab.binary),'--config',str(config)],capture_output=True,timeout=5)
    assert result.returncode!=0 and b'integrity mismatch' in result.stderr and not (lab.root/'never-created').exists()
def malformed(lab):
    folder,manifest=fixture(lab.root)
    config=lab.root/'bad.toml';config.write_text("data_dir='never-created'\nhttp_bind='127.0.0.1:0'\ndownloads_manifest='artifacts/manifest.toml'\n")
    invalid=[manifest.replace('schema=1','schema=2'),manifest.replace('file="client.exe"','file="../client.exe"'),manifest.replace('kind="client"','kind="unknown"'),manifest.replace('size='+str(len(CONTENT)),'size=33554433'),manifest.replace('source_url="https://example.invalid/source"','source_url="javascript:alert(1)"'),manifest+'unknown="SYNTHETIC_CANARY"\n',manifest+manifest[manifest.index('[[artifacts]]'):], 'schema=1\nartifacts=[]\n', 'x'*16385]
    for text in invalid:
        (folder/'manifest.toml').write_text(text,encoding='utf-8');result=subprocess.run([str(lab.binary),'--config',str(config)],capture_output=True,timeout=5)
        assert result.returncode!=0 and not (lab.root/'never-created').exists() and b'SYNTHETIC_CANARY' not in result.stderr
    (lab.root/'outside.bin').write_bytes(CONTENT)
    (folder/'manifest.toml').write_text(manifest,encoding='utf-8');(folder/'client.exe').unlink();(folder/'client.exe').symlink_to(lab.root/'outside.bin')
    assert subprocess.run([str(lab.binary),'--config',str(config)],capture_output=True,timeout=5).returncode!=0
def downloads_only_and_budget(lab):
    fixture(lab.root);lab.start(downloads='artifacts/manifest.toml')
    assert request(lab,'/')[0]==404 and request(lab,'/register')[0]==404 and request(lab,'/api/stats')[0]==404
    for _ in range(60):assert request(lab,'/downloads/client')[0]==200
    assert request(lab,'/downloads/client',headers={'X-Forwarded-For':'192.0.2.2'})[0]==429
    assert request(lab,'/health/ready')[0]==200;lab.client(1).ping()
def slow_readers(lab):
    fixture(lab.root,content=b'Z'*(16*1024*1024));lab.start(downloads='artifacts/manifest.toml')
    connections=[]
    try:
        host,port=lab.server.address.split(':')
        for _ in range(4):
            c=socket.socket();c.setsockopt(socket.SOL_SOCKET,socket.SO_RCVBUF,1024);c.settimeout(3);c.connect((host,int(port)))
            c.sendall(('GET /downloads/client HTTP/1.1\r\nHost: '+lab.server.address+'\r\nConnection: close\r\n\r\n').encode())
            header=b''
            while b'\r\n\r\n' not in header:header+=c.recv(1)
            assert header.startswith(b'HTTP/1.1 200');connections.append(c)
        assert request(lab,'/downloads/client')[0]==429
        assert request(lab,'/health/ready')[0]==200;lab.client(1).ping()
    finally:
        for c in connections:c.close()
    time.sleep(.2)
    assert request(lab,'/downloads/client',method='HEAD')[0]==200
def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);args=p.parse_args()
    for name,test in [('hash verified immutable snapshot, HTML escaping, headers and integration',integrity),('bad manifests, traversal, duplicates, sizes and symlinks fail before startup',malformed),('independent opt-in, budget and GG isolation',downloads_only_and_budget),('four slow streams bounded, disconnect releases capacity',slow_readers)]:
        with tempfile.TemporaryDirectory(prefix='rgg-downloads-') as root:
            lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
            try:test(lab);print('PASS:',name,flush=True)
            finally:lab.stop()
if __name__=='__main__':main()
