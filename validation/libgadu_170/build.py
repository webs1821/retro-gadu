# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 webs1821
"""Build an external pinned source archive and the standalone probe, never install it."""
import argparse, hashlib, json, os, socket, subprocess, tarfile, time
from pathlib import Path, PurePosixPath

SOURCE_URL='https://github.com/wojtekka/libgadu/releases/download/1.7.0/libgadu-1.7.0.tar.gz'
SOURCE_SHA256='13b0c4c1471f40022db3fd7d141d5d0b42dd67f0a9ca82db877c39b60e14071a'

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--archive',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True); parser.add_argument('--outer-network-namespace',required=True)
    args=parser.parse_args(); namespace=os.readlink('/proc/self/ns/net')
    assert namespace!=args.outer_network_namespace and args.outer_network_namespace.startswith('net:[')
    assert {name for _,name in socket.if_nameindex()}=={'lo'}
    assert args.archive.stat().st_size==382784 and digest(args.archive)==SOURCE_SHA256
    assert not args.output.exists(); args.output.mkdir(mode=0o700)
    output=args.output.resolve(); probe_source=Path(__file__).with_name('probe.c').resolve()
    with tarfile.open(args.archive,'r:gz') as archive:
        members=archive.getmembers(); assert len(members)<5000 and sum(m.size for m in members)<32*1024*1024
        assert all(not PurePosixPath(m.name).is_absolute() and '..' not in PurePosixPath(m.name).parts and (m.isfile() or m.isdir()) for m in members)
        # Links and unsafe member paths are rejected above, including on Python 3.10.
        archive.extractall(output)
    source=output/'libgadu-1.7.0'
    original={str(p.relative_to(source)):digest(p) for p in source.rglob('*') if p.is_file()}
    commands=[['sh','./configure','--disable-shared','--enable-static','--without-openssl','--without-pthread','--without-bind','CFLAGS=-O1 -g'],
              ['make','-j2'],
              ['/usr/bin/gcc','-std=c11','-Wall','-Wextra','-Werror','-O1','-g','-I'+str(source/'include'),str(probe_source),str(source/'src/.libs/libgadu.a'),'-o',str(output/'probe')]]
    report={'source_url':SOURCE_URL,'archive_sha256':SOURCE_SHA256,'passed':False,'steps':[],
            'builder_sha256':digest(Path(__file__)),'probe_source_sha256':digest(probe_source),
            'network_namespace':namespace,'outer_network_namespace':args.outer_network_namespace,
            'installed_systemwide':False}
    environment=os.environ.copy(); environment.update(CC='/usr/bin/gcc',CONFIG_SITE='/dev/null')
    try:
        for index,command in enumerate(commands):
            start=time.monotonic(); result=subprocess.run(command,cwd=source,env=environment,capture_output=True,timeout=120)
            log=output/f'build-{index}.log'; log.write_bytes(result.stdout+result.stderr)
            report['steps'].append({'command':command,'returncode':result.returncode,'seconds':round(time.monotonic()-start,3),'log_sha256':digest(log)})
            assert result.returncode==0,log.read_text(errors='replace')[-3000:]
        assert all(digest(source/name)==value for name,value in original.items()),'upstream sources changed'
        report.update(passed=True,original_sources_unchanged=True,upstream_files_sha256=original,
                      static_library_sha256=digest(source/'src/.libs/libgadu.a'),probe_sha256=digest(output/'probe'),
                      compiler=subprocess.check_output(['/usr/bin/gcc','--version'],text=True).splitlines()[0])
    finally:
        (output/'build.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print('PASS: unchanged libgadu 1.7.0 built privately; probe='+report['probe_sha256'],flush=True)

if __name__=='__main__':main()
