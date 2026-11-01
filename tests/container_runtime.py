# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Isolated runtime image, persistent volume, restart and backup/restore on an explicit local engine."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import uuid

LIMITS = ['--memory', '256m', '--memory-swap', '256m', '--cpus', '1', '--pids-limit', '128',
          '--read-only', '--tmpfs', '/tmp:rw,nosuid,nodev,noexec,size=32m',
          '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges=true']
READY = '''import http.client,json,time
for attempt in range(50):
 try:
  c=http.client.HTTPConnection('127.0.0.1',8088,timeout=1);c.request('GET','/health/ready');r=c.getresponse();assert r.status==200;r.read();c.close();break
 except (OSError,AssertionError):time.sleep(.1)
else:raise AssertionError('container did not become ready')
import sys
sys.path.insert(0,'/src/tests')
from gg6_sessions import Client
'''


class Engine:
    def __init__(self, args, directory):
        self.cli = [str(args.docker.resolve()), '--config', str(directory), '--host', 'unix://' + str(args.socket)]
        self.env = {k: v for k, v in os.environ.items() if not k.startswith('DOCKER_')}
        self.label = 'org.retrogadu.test-run'
        self.run_id = 'rgg-' + uuid.uuid4().hex[:16]
        self.containers, self.volumes = [], []

    def call(self, args, input=None, check=True, timeout=180):
        return subprocess.run(self.cli + list(args), input=input, capture_output=True, text=True,
                              env=self.env, check=check, timeout=timeout)

    def inspect(self, kind, name):
        return json.loads(self.call([kind, 'inspect', name]).stdout)[0]

    def volume(self):
        name = self.run_id + '-v' + str(len(self.volumes))
        self.call(['volume', 'create', '--driver', 'local', '--label', self.label + '=' + self.run_id, name])
        self.volumes.append(name)
        return name

    def run(self, image, command=(), mounts=(), network='none', input=None, detached=False, check=True):
        name = self.run_id + '-c' + str(len(self.containers))
        self.containers.append(name)
        options = ['run', '--name', name, '--label', self.label + '=' + self.run_id, '--network', network, *LIMITS]
        options += ['-d'] if detached else ['--rm', '-i']
        for volume, target, readonly in mounts:
            options += ['--mount', 'type=volume,source=' + volume + ',target=' + target + (',readonly' if readonly else '')]
        result = self.call([*options, image, *command], input=input, check=check)
        return name if detached else result

    def stop(self, name):
        self.call(['stop', '--time', '10', name])
        state = self.inspect('container', name)['State']
        assert state['ExitCode'] == 0 and not state['OOMKilled']

    def cleanup(self):
        for kind, names in [('container', self.containers), ('volume', self.volumes)]:
            for name in reversed(names):
                result = self.call([kind, 'inspect', name], check=False)
                if result.returncode:
                    continue
                item = json.loads(result.stdout)[0]
                labels = item['Config']['Labels'] if kind == 'container' else item['Labels']
                assert labels.get(self.label) == self.run_id, 'refusing cleanup of unrelated resource'
                self.call([kind, 'rm', *(['-f'] if kind == 'container' else []), name])


def qualify(args, engine):
    runtime = engine.inspect('image', args.runtime)
    helper = engine.inspect('image', args.verification)
    revision = runtime['Config']['Labels']['org.opencontainers.image.revision']
    assert helper['Config']['Labels']['org.opencontainers.image.revision'] == revision
    assert all(image['Config']['User'] == '10001:10001' for image in [runtime, helper])
    assert all(image['Config']['Labels'].get('org.retrogadu.local-test') == 'true' for image in [runtime, helper])
    data, backup, restored = [engine.volume() for _ in range(3)]
    source_mount = [(data, '/data', False)]
    admin = ['/usr/local/bin/rgg-admin']
    engine.run(args.runtime, admin + ['init', '--data-dir', '/data'], source_mount)
    bootstrap = '''import subprocess,json
def command(name,payload=None):
 r=subprocess.run(['/usr/local/bin/rgg-admin',name,'--data-dir','/data'],input=json.dumps(payload) if payload else None,text=True,capture_output=True,check=True);return json.loads(r.stdout)
code=command('invite')['codes'][0]
for uin in (1,2,3):command('account-create',dict(requested_uin=uin,invite=code,password='LocalTest9!',email='synthetic@example.invalid'))
assert command('stats')['registered']==3
print('synthetic accounts prepared')
'''
    engine.run(args.verification, ['python', '-c', bootstrap], source_mount)
    server = engine.run(args.runtime, mounts=source_mount, detached=True)
    config = engine.inspect('container', server)['HostConfig']
    assert config['ReadonlyRootfs'] and config['NetworkMode'] == 'none' and not config['PortBindings']
    assert config['Memory'] == 268435456 and config['NanoCpus'] == 1000000000 and config['PidsLimit'] == 128
    probe = engine.call(['exec', server, 'cat', '/sys/fs/cgroup/memory.max', '/sys/fs/cgroup/cpu.max', '/sys/fs/cgroup/pids.max', '/sys/fs/cgroup/memory.swap.max']).stdout.splitlines()
    assert probe[:3] == ['268435456', '100000 100000', '128']
    proc = engine.call(['exec', server, 'cat', '/proc/1/status']).stdout
    status = dict(line.split(':', 1) for line in proc.splitlines() if ':' in line)
    assert status['Uid'].split() == ['10001'] * 4 and status['Gid'].split() == ['10001'] * 4
    assert int(status['CapEff'].strip(), 16) == 0 and status['NoNewPrivs'].strip() == '1' and status['Seccomp'].strip() == '2'
    binary_hash = engine.call(['exec', server, 'sha256sum', '/usr/local/bin/rgg-server']).stdout.split()[0]
    before = READY + "a=Client(('127.0.0.1',8074));assert a.login(1)==(3,b'');assert a.message(2,42,'Container restart fixture')==3;a.close();print('queued')"
    engine.run(args.verification, ['python', '-c', before], network='container:' + server)
    locked = engine.run(args.runtime, admin + ['stats', '--data-dir', '/data'], source_mount, check=False)
    assert locked.returncode != 0 and 'another process owns' in locked.stderr
    engine.stop(server)
    engine.call(['start', server])
    after = READY + "b=Client(('127.0.0.1',8074));assert b.login(2)==(3,b'');kind,body=b.receive();assert kind==10 and body[16:]==b'Container restart fixture\\0';b.ping();b.close();print('persisted delivery verified')"
    engine.run(args.verification, ['python', '-c', after], network='container:' + server)
    engine.stop(server)
    engine.run(args.runtime, admin + ['backup-create', '--data-dir', '/source', '--output', '/data/snapshot'], [(data, '/source', False), (backup, '/data', False)])
    engine.run(args.runtime, admin + ['backup-verify', '--backup', '/data/snapshot'], [(backup, '/data', False)])
    engine.run(args.runtime, admin + ['restore', '--backup', '/source/snapshot', '--data-dir', '/data/restored'], [(backup, '/source', True), (restored, '/data', False)])
    command = ['sh', '-c', 'sed \'s|data_dir = "/data"|data_dir = "/data/restored"|\' /etc/rgg/rgg.toml > /tmp/rgg.toml && exec /usr/local/bin/rgg-server --config /tmp/rgg.toml']
    restored_server = engine.run(args.runtime, command, [(restored, '/data', False)], detached=True)
    roundtrip = READY + "a=Client(('127.0.0.1',8074));b=Client(('127.0.0.1',8074));assert a.login(1)==(3,b'') and b.login(2)==(3,b'');assert a.message(2,99,'Restored volume')==3;kind,body=b.receive();assert kind==10 and body[16:]==b'Restored volume\\0';a.close();b.close();print('restored authentication and write verified')"
    engine.run(args.verification, ['python', '-c', roundtrip], network='container:' + restored_server)
    engine.stop(restored_server)
    return dict(format=1, passed=True, source_commit=revision, runtime_image_id=runtime['Id'], verification_image_id=helper['Id'],
                binary_sha256=binary_hash, checks=['non-root PID1', 'effective cgroup limits', 'capabilities/seccomp/no-new-privileges',
                'no network or published ports', 'named volume initialization', 'live CLI exclusion', 'offline delivery after container restart',
                'backup/verify/restore into a new volume', 'restored authentication and message write', 'graceful SIGTERM'],
                cpu=1, memory_max=268435456, memory_swap_max=probe[3], swap_disabled=probe[3] == '0', pids_max=128,
                native_client_qualified=False, capacity_r0_qualified=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--docker', type=Path, required=True)
    parser.add_argument('--socket', type=Path, required=True)
    parser.add_argument('--runtime', required=True)
    parser.add_argument('--verification', required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert args.socket.is_absolute() and args.socket.is_socket() and not args.report.exists()
    assert all(re.fullmatch(r'sha256:[0-9a-f]{64}', value) for value in [args.runtime, args.verification])
    with tempfile.TemporaryDirectory(prefix='rgg-engine-client-') as tmp:
        engine = Engine(args, Path(tmp))
        try:
            report = qualify(args, engine)
        finally:
            engine.cleanup()
    report['temporary_containers_and_volumes_removed'] = True
    with args.report.open('x', encoding='utf-8') as output:
        json.dump(report, output, indent=2); output.write('\n')
    print(json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
