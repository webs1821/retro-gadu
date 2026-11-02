# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""R0 on an explicit local engine; read actual cgroups and remove only owned resources."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import tempfile
import time
from container_runtime import Engine, LIMITS, READY

MIB=1024*1024

def fields(path):
    return {line.split()[0]:int(line.split()[1]) for line in path.read_text().splitlines()}

def cgroup(engine, server):
    info=engine.inspect('container',server)
    assert info['Config']['Labels'][engine.label]==engine.run_id
    assert info['State']['Running'] and info['State']['Pid']>0
    pid=info['State']['Pid']
    text=Path('/proc/%d/cgroup'%pid).read_text()
    group=[line[3:] for line in text.splitlines() if line.startswith('0::')]
    assert len(group)==1 and group[0].startswith('/')
    root=Path('/sys/fs/cgroup').resolve()
    path=(root/group[0].lstrip('/')).resolve(strict=True)
    path.relative_to(root)
    assert info['Id'] in str(path), 'cgroup must belong to this exact container ID'
    status=dict(line.split(':',1) for line in Path('/proc/%d/status'%pid).read_text().splitlines() if ':' in line)
    assert set(status['Uid'].split())=={'10001'} and set(status['Gid'].split())=={'10001'}
    assert int(status['CapEff'].strip(),16)==0 and status['NoNewPrivs'].strip()=='1' and status['Seccomp'].strip()=='2'
    assert (path/'memory.max').read_text().strip()==str(256*MIB)
    assert (path/'cpu.max').read_text().split()==['100000','100000']
    assert (path/'pids.max').read_text().strip()=='128'
    swap_before=(path/'memory.swap.max').read_text().strip()
    if swap_before!='0':
        # Recheck ownership/PID immediately before changing only our disposable cgroup.
        again=engine.inspect('container',server)
        assert again['Id']==info['Id'] and again['State']['Running'] and again['State']['Pid']==pid
        assert Path('/proc/%d/cgroup'%pid).read_text()==text
        (path/'memory.swap.max').write_text('0\n')
    assert (path/'memory.swap.max').read_text().strip()=='0'
    return pid,path,dict(memory_max=256*MIB,cpu_max='100000 100000',pids_max=128,swap_max=0,
                         swap_initial=swap_before,swap_set_directly=swap_before!='0',uid=10001,read_only_root=True,
                         cap_eff=0,no_new_privileges=True,seccomp_filter=True)

def sample(pid,group):
    status=dict(line.split(':',1) for line in Path('/proc/%d/status'%pid).read_text().splitlines() if ':' in line)
    start_ticks=int(Path('/proc/%d/stat'%pid).read_text().rsplit(')',1)[1].split()[19])
    return dict(at=time.monotonic(),pid=pid,start_ticks=start_ticks,fds=len(list(Path('/proc/%d/fd'%pid).iterdir())),rss=int(status['VmRSS'].split()[0])*1024,
                rss_peak=int(status['VmHWM'].split()[0])*1024,threads=int(status['Threads']),
                memory_current=int((group/'memory.current').read_text()),memory_peak=int((group/'memory.peak').read_text()),
                swap_current=int((group/'memory.swap.current').read_text()),pids_current=int((group/'pids.current').read_text()),
                cpu=fields(group/'cpu.stat'),memory_events=fields(group/'memory.events'))

def qualify(args,engine,tmp,evidence):
    runtime=engine.inspect('image',args.runtime);helper=engine.inspect('image',args.verification)
    revision=runtime['Config']['Labels']['org.opencontainers.image.revision']
    evidence.update(source_commit=revision,runtime_image=args.runtime,verification_image=args.verification)
    assert helper['Config']['Labels']['org.opencontainers.image.revision']==revision
    assert all(image['Config']['User']=='10001:10001' and image['Config']['Labels'].get('org.retrogadu.local-test')=='true' for image in [runtime,helper])
    volume=engine.volume();mount=[(volume,'/data',False)]
    # Docker copies /data's owner/mode from the runtime image on the first mount.
    # The Python-only verification image deliberately has no initialized /data.
    engine.run(args.runtime,['/usr/bin/test','-w','/data'],mount)
    engine.run(args.verification,['python','/src/tests/r0_workload.py','prepare','--admin','/usr/local/bin/rgg-admin','--data','/data/runtime','--count','120'],mount)
    config="data_dir='/data/runtime'\nhttp_bind='127.0.0.1:8088'\ngg_bind='127.0.0.1:8074'\nmetrics_bind='127.0.0.1:9090'\n[network]\nmax_connections=128\nper_ip=128\n"
    engine.run(args.verification,['python','-c',"from pathlib import Path;Path('/data/r0.toml').write_text("+repr(config)+")"],mount)
    server=engine.run(args.runtime,['/usr/local/bin/rgg-server','--config','/data/r0.toml'],mount,detached=True)
    host=engine.inspect('container',server)['HostConfig']
    assert host['NetworkMode']=='none' and not host['PortBindings'] and host['ReadonlyRootfs']
    assert host['Memory']==256*MIB and host['NanoCpus']==1000000000 and host['PidsLimit']==128
    pid,group,limits=cgroup(engine,server)
    evidence['limits']=limits
    engine.run(args.verification,['python','-c',READY],network='container:'+server)
    baseline=sample(pid,group)
    print('R0_EVENT='+json.dumps({'phase':'start','source_commit':revision,'limits':limits}),flush=True)
    client=engine.run_id+'-load';engine.containers.append(client)
    workload_source=Path(__file__).with_name('r0_workload.py').read_bytes()
    workload_sha256=hashlib.sha256(workload_source).hexdigest()
    evidence['workload_driver_sha256']=workload_sha256
    runner="import sys;exec(compile(sys.stdin.buffer.read(),'/src/tests/r0_workload.py','exec'),{'__name__':'__main__','__file__':'/src/tests/r0_workload.py'})"
    command=engine.cli+['run','--rm','-i','--name',client,'--label',engine.label+'='+engine.run_id,
                        '--network','container:'+server,*LIMITS,args.verification,'python','-u','-c',runner,'run','--seconds',str(args.seconds),'--cycles',str(args.cycles)]
    log=tmp/'load.log';errors=tmp/'load.stderr';samples=[baseline]
    evidence['resource_samples']=samples
    with log.open('w') as output,errors.open('w') as error_output:
        process=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=output,stderr=error_output,env=engine.env)
        try:
            process.stdin.write(workload_source);process.stdin.close()
            deadline=time.monotonic()+args.cycles*(args.seconds+60)+30
            last_notice=time.monotonic()
            while process.poll() is None:
                assert time.monotonic()<deadline,'load process deadline exceeded'
                samples.append(sample(pid,group))
                if time.monotonic()-last_notice>=30:
                    last_notice=time.monotonic()
                    print('R0_EVENT='+json.dumps({'phase':'sampling','rss_mib':round(samples[-1]['rss']/MIB,2),'samples':len(samples)}),flush=True)
                time.sleep(1)
            assert process.returncode==0, 'load driver failed: '+log.read_text()[-3000:]+' '+errors.read_text()[-3000:]
        finally:
            if process.poll() is None:
                process.terminate()
                try:process.wait(timeout=5)
                except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
            evidence['load_log_tail']=log.read_text()[-5000:]
            evidence['load_stderr_tail']=errors.read_text()[-5000:]
    samples.append(sample(pid,group))
    reports=[json.loads(line.split('=',1)[1]) for line in log.read_text().splitlines() if line.startswith('LOAD_REPORT=')]
    assert len(reports)==args.cycles
    evidence['workload_cycles']=reports
    phases=[];cpu_fractions=[]
    for index,workload in enumerate(reports):
        assert workload['initial_terminal_records']==index*args.seconds*10
        assert workload['sequence_base']==index*args.seconds*10
        phase=[item for item in samples if workload['measure_start']<=item['at']<=workload['measure_end']]
        assert len(phase)>2 and phase[-1]['at']-phase[0]['at']>=args.seconds-3,'insufficient resource coverage or inconsistent clocks'
        phases.append(phase)
        cpu_seconds=(phase[-1]['cpu']['usage_usec']-phase[0]['cpu']['usage_usec'])/1000000
        cpu_fractions.append(cpu_seconds/(phase[-1]['at']-phase[0]['at']))
    cpu_fraction=max(cpu_fractions)
    final=samples[-1]
    gates=dict(all_messages=all(w['accepted']==w['scheduled']==args.seconds*10 for w in reports),
               exact_payloads=all(w['all_payloads_exact'] and w['offline_fifo'] for w in reports),
               session_samples=all(len(w['metrics_samples'])>=args.seconds//2 and all(item['ready_sessions']==100 for item in w['metrics_samples']) for w in reports),
               rate=all(w['achieved_rate']>=9.9 for w in reports),schedule=all(w['schedule_lag_max_ms']<=250 for w in reports),
               ack=all(w['ack_p95_ms']<=250 and w['ack_max_ms']<1000 for w in reports),online=all(w['online_p95_ms']<=500 for w in reports),
               rss=max(item['rss_peak'] for item in samples)<=192*MIB,cpu=cpu_fraction<=.90,
               swap=all(item['swap_current']==0 for item in samples),
               same_process=all((item['pid'],item['start_ticks'])==(baseline['pid'],baseline['start_ticks']) for item in samples),
               no_oom=all(final['memory_events'][key]==baseline['memory_events'][key] for key in ['oom','oom_kill']))
    quiet=[]
    if args.cycles>1:
        summaries=[json.loads(line.split('=',1)[1]) for line in log.read_text().splitlines() if line.startswith('SOAK_REPORT=')]
        assert len(summaries)==1
        summary=summaries[0]
        assert summary['cycles']==reports and summary['total_verified']==args.cycles*args.seconds*10
        assert len(summary['quiescent_intervals'])==args.cycles
        for interval in summary['quiescent_intervals']:
            observed=[item for item in samples if interval['end']-3<=item['at']<=interval['end']]
            assert len(observed)>=2 and interval['end']-interval['start']>=5, 'insufficient quiescent samples'
            quiet.append(dict(cycle=interval['cycle'],sample_count=len(observed),start=interval['start'],end=interval['end'],
                              rss_median=statistics.median(item['rss'] for item in observed),
                              fds_median=statistics.median(item['fds'] for item in observed),
                              threads_median=statistics.median(item['threads'] for item in observed)))
        gates.update(rss_growth=quiet[-1]['rss_median']-quiet[0]['rss_median']<=16*MIB,
                     fd_growth=quiet[-1]['fds_median']-quiet[0]['fds_median']<=4,
                     thread_growth=quiet[-1]['threads_median']-quiet[0]['threads_median']<=4)
        evidence['quiescent_samples']=quiet
    evidence.update(gates_before_database=gates.copy(),cpu_fraction=cpu_fraction,peak_rss_bytes=max(item['rss_peak'] for item in samples))
    engine.stop(server)
    inspection=engine.volume()
    engine.run(args.runtime,['/usr/bin/test','-w','/data'],[(inspection,'/data',False)])
    verify="""import sqlite3,json,tempfile,shutil
from pathlib import Path
# Inspect a stable copy after exit 0. SQLite may need new WAL/SHM sidecars;
# immutable=1 would risk ignoring committed WAL, while the source stays read-only.
temporary=tempfile.TemporaryDirectory(prefix='rgg-inspect-',dir='/data')
target=Path(temporary.name);source=Path('/source/runtime');total=0
for name in ['rgg.sqlite','rgg.sqlite-wal']:
 path=source/name
 if not path.exists():
  assert name.endswith('-wal');continue
 assert path.is_file() and not path.is_symlink()
 total+=path.stat().st_size;assert total<=64*1024*1024
 shutil.copyfile(path,target/name)
db=sqlite3.connect(str(target/'rgg.sqlite'))
assert db.execute('PRAGMA integrity_check').fetchone()==('ok',)
assert not db.execute('PRAGMA foreign_key_check').fetchall()
assert db.execute('SELECT count(*) FROM account_sessions').fetchone()==(0,)
assert db.execute('SELECT count(*) FROM delivery_attempts WHERE completed_at IS NULL').fetchone()==(0,)
assert db.execute('SELECT count(*) FROM messages WHERE body IS NOT NULL OR styles IS NOT NULL OR conference IS NOT NULL').fetchone()==(0,)
counts=dict(db.execute('SELECT state,count(*) FROM deliveries GROUP BY state').fetchall());db.close();temporary.cleanup()
print(json.dumps(counts))
"""
    database=json.loads(engine.run(args.verification,['python','-c',verify],[(volume,'/source',True),(inspection,'/data',False)]).stdout)
    gates['database']=database=={'TransportWrittenUnconfirmed':args.cycles*args.seconds*10}
    report=dict(format=1,source_commit=revision,version=runtime['Config']['Labels']['org.opencontainers.image.version'],
                runtime_image=args.runtime,verification_image=args.verification,limits=limits,
                workload=reports[0] if args.cycles==1 else None,workload_cycles=reports,quiescent_samples=quiet,
                resource_samples=samples,cpu_fraction=round(cpu_fraction,6),cpu_fractions=cpu_fractions,
                cpu_observation_seconds=[round(phase[-1]['at']-phase[0]['at'],3) for phase in phases],
                peak_rss_bytes=max(item['rss_peak'] for item in samples),memory_peak_bytes=max(item['memory_peak'] for item in samples),
                database=database,gates=gates,passed=all(gates.values()),
                r0_sample_qualified=args.seconds>=180 and all(gates.values()),
                soak_qualified=args.cycles==4 and args.seconds>=900 and all(gates.values()),native_client_qualified=False,production_qualified=False,
                workload_driver_sha256=workload_sha256,
                driver_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    return report

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--docker',type=Path,required=True);p.add_argument('--socket',type=Path,required=True)
    p.add_argument('--runtime',required=True);p.add_argument('--verification',required=True)
    p.add_argument('--seconds',type=int,default=180);p.add_argument('--report',type=Path,required=True)
    p.add_argument('--cycles',type=int,choices=range(1,5),default=1)
    args=p.parse_args()
    assert os.name=='posix' and args.socket.is_absolute() and args.socket.is_socket() and not args.report.exists()
    assert 10<=args.seconds<=1200
    assert all(re.fullmatch(r'sha256:[a-f0-9]{64}',value) for value in [args.runtime,args.verification])
    report=None;failure=None;evidence={}
    with tempfile.TemporaryDirectory(prefix='rgg-container-load-') as temporary:
        tmp=Path(temporary);client=tmp/'client';client.mkdir();engine=Engine(args,client)
        try:report=qualify(args,engine,tmp,evidence)
        except BaseException as error:
            failure=error;report=dict(format=1,passed=False,r0_sample_qualified=False,soak_qualified=False,error=type(error).__name__+': '+str(error),evidence=evidence)
            if isinstance(error,subprocess.CalledProcessError):report['command_stderr']=(error.stderr or '')[-5000:]
        finally:
            try:
                engine.cleanup()
                assert not engine.call(['ps','-a','--filter','label='+engine.label+'='+engine.run_id,'--format','{{.ID}}']).stdout.strip()
                assert not engine.call(['volume','ls','--filter','label='+engine.label+'='+engine.run_id,'--format','{{.Name}}']).stdout.strip()
                report['cleanup_verified']=True
            except BaseException as error:
                failure=failure or error;report.update(passed=False,r0_sample_qualified=False,soak_qualified=False,cleanup_verified=False,cleanup_error=str(error))
        with args.report.open('x') as output:json.dump(report,output,indent=2);output.write('\n')
    print(json.dumps({key:report.get(key) for key in ['passed','r0_sample_qualified','soak_qualified','cleanup_verified','peak_rss_bytes','cpu_fraction','gates']}),flush=True)
    if failure:raise failure
    if not report['passed']:raise SystemExit('R0 gates failed; inspect report')
if __name__=='__main__':main()
