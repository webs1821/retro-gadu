# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Build a tracked-only image on an explicit local Unix Docker socket (Linux, Python >=3.8).
No push, deployment, daemon start, user registry credentials or implicit Docker context.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def git_environment(directory):
    env = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
    config = directory / 'gitconfig'
    env.update(GIT_CONFIG_GLOBAL=str(config), GIT_CONFIG_NOSYSTEM='1', GIT_OPTIONAL_LOCKS='0', GIT_TERMINAL_PROMPT='0')
    subprocess.run(['git', 'config', '--file', str(config), '--add', 'safe.directory', str(ROOT)], env=env, check=True)
    return env


def source(git_env=None):
    git = ['git', '-c', 'safe.directory=' + str(ROOT), '-C', str(ROOT)]
    if subprocess.run(git + ['diff', '--quiet', 'HEAD'], env=git_env).returncode:
        raise ValueError('tracked source must be committed before building')
    commit = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True, env=git_env).strip()
    cargo = subprocess.check_output(git + ['show', 'HEAD:Cargo.toml'], text=True, env=git_env)
    versions = re.findall(r'^version = "([0-9A-Za-z.+-]{1,64})"$', cargo, re.MULTILINE)
    if len(versions) != 1 or not re.fullmatch('[a-f0-9]{40}', commit):
        raise ValueError('source identity is ambiguous')
    return git, commit, versions[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--docker', type=Path, required=True)
    parser.add_argument('--socket', type=Path, required=True)
    parser.add_argument('--target', choices=['runtime', 'verification'], default='runtime')
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    if os.name != 'posix' or not args.socket.is_absolute() or not args.socket.is_socket():
        raise ValueError('an explicit existing local Unix socket is required')
    if args.report.exists():
        raise ValueError('report must be new')
    env = os.environ.copy()
    for key in ['DOCKER_HOST', 'DOCKER_CONTEXT', 'DOCKER_TLS_VERIFY', 'DOCKER_CERT_PATH', 'DOCKER_AUTH_CONFIG', 'DOCKER_CONFIG']:
        env.pop(key, None)
    # This is a portable local development path; no BuildKit daemon is assumed.
    env['DOCKER_BUILDKIT'] = '0'
    with tempfile.TemporaryDirectory(prefix='rgg-container-build-') as tmp:
        git_env = git_environment(Path(tmp))
        git, commit, version = source(git_env)
        tag = 'rgg-local:' + commit[:12] + '-' + args.target
        client = Path(tmp) / 'client'; client.mkdir()
        cli = [str(args.docker.resolve()), '--config', str(client), '--host', 'unix://' + str(args.socket)]
        engine = json.loads(subprocess.check_output(cli + ['info', '--format', '{{json .}}'], env=env))
        if engine['OSType'] != 'linux':
            raise ValueError('Linux engine required')
        existing = subprocess.run(cli + ['image', 'inspect', tag], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if existing.returncode == 0:
            raise ValueError('this source/target image tag already exists; refusing overwrite')
        archive = Path(tmp) / 'source.tar'
        with archive.open('xb') as output:
            subprocess.run(git + ['archive', '--format=tar', commit], stdout=output, env=git_env, check=True)
        with archive.open('rb') as context:
            subprocess.run(cli + ['build', '--network=host', '--memory=3g', '--memory-swap=3g',
                                 '--cpu-period=100000', '--cpu-quota=200000', '--target', args.target,
                                 '--build-arg', 'SOURCE_COMMIT=' + commit, '--build-arg', 'VERSION=' + version,
                                 '--label', 'org.retrogadu.local-test=true', '-t', tag,
                                 '-f', 'deploy/Dockerfile', '-'], stdin=context, env=env, check=True)
        info = json.loads(subprocess.check_output(cli + ['image', 'inspect', tag], env=env))[0]
        if info['Config']['User'] != '10001:10001' or info['Config']['Labels']['org.opencontainers.image.revision'] != commit:
            raise ValueError('built image identity/user mismatch')
        report = dict(format=1, built=True, source_commit=commit, version=version, target=args.target,
                      image=tag, image_id=info['Id'], architecture=info['Architecture'],
                      engine_version=engine['ServerVersion'], image_size=info['Size'],
                      source_archive='git archive of the recorded commit; vendored Cargo sources included in runtime',
                      pushed=False, deployed=False, runtime_qualified=False)
        with args.report.open('x', encoding='utf-8') as output:
            json.dump(report, output, indent=2); output.write('\n')
        print(json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
