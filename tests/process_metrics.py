# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Real Linux process observations, private HTTP and concurrent GG; synthetic data only."""
import argparse,os,tempfile,time
from pathlib import Path
from gg6_sessions import Lab,until
from http_metrics import metrics

CPU='rgg_process_cpu_seconds_total'
RSS='rgg_process_resident_memory_bytes'
AGE='rgg_process_sample_age_seconds'

def ready(lab):
    def available():
        sample=metrics(lab)
        return CPU in sample and RSS in sample
    until(available)
    return metrics(lab)

def compare_kernel(lab,sample):
    root=Path('/proc')/str(lab.server.process.pid)
    stat=(root/'stat').read_text()
    fields=stat[stat.rfind(')')+2:].split()
    ticks=os.sysconf('SC_CLK_TCK')
    cpu=(int(fields[11])+int(fields[12]))/ticks
    # Independent kernel counters have tick rounding; the exporter uses a clock syscall.
    assert 0<sample[CPU]<=cpu+2/ticks,(sample[CPU],cpu)
    status=(root/'status').read_bytes().splitlines()
    resident=int(next(line.split()[1] for line in status if line.startswith(b'VmRSS:')))*1024
    # RSS is asynchronous, and the one-second sample may precede request allocations.
    assert resident//2<=sample[RSS]<=resident*2 and sample[RSS]>1024*1024,(sample[RSS],resident)
    assert 0<=sample[AGE]<=5

def flow(lab):
    lab.start()
    assert 'Private process measurements enabled' not in lab.server.log.read_text()
    lab.stop()
    assert 'Process measurement sampler stopped' not in lab.server.log.read_text()
    lab.start(metrics=True)
    first_pid=lab.server.process.pid
    first=ready(lab);compare_kernel(lab,first)
    client=lab.client(1)
    for _ in range(10):client.ping()
    until(lambda:ready(lab)[CPU]>first[CPU])
    second=ready(lab);compare_kernel(lab,second)
    assert len(second)==117
    client.ping()
    started=time.monotonic();lab.stop()
    assert time.monotonic()-started<5
    log=lab.server.log.read_text()
    assert log.count('Private process measurements enabled')==1
    assert log.count('Process measurement sampler stopped')==1
    assert 'sampler failed' not in log and 'sampler drain deadline' not in log
    assert lab.server.process.returncode==0
    lab.start(metrics=True)
    restarted=ready(lab);compare_kernel(lab,restarted)
    assert lab.server.process.pid!=first_pid # the counter is checked against this new process above
    lab.client(1).ping()
    lab.stop()
    assert lab.server.log.read_text().count('Process measurement sampler stopped')==1
    assert lab.sql('PRAGMA integrity_check')==[('ok',)]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--server',type=Path,required=True);parser.add_argument('--admin',type=Path,required=True);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='rgg-process-metrics-') as root:
        lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
        try:
            flow(lab)
            print('PASS: process CPU/RSS units, sampling, opt-in, simultaneous GG, restart and joined shutdown',flush=True)
        finally:lab.stop()

if __name__=='__main__':main()
