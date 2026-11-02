# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Run from stdin inside the isolated verification image; validates its actual release binaries."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import time


def main():
    assert os.getuid() == 10001 and os.getgid() == 10001
    assert os.statvfs('/').f_flag & os.ST_RDONLY
    group = Path('/sys/fs/cgroup')
    assert (group / 'memory.max').read_text().strip() == '268435456'
    assert (group / 'cpu.max').read_text().split() == ['100000', '100000']
    assert (group / 'pids.max').read_text().strip() == '128'
    status = dict(line.split(':', 1) for line in Path('/proc/self/status').read_text().splitlines() if ':' in line)
    assert int(status['CapEff'].strip(), 16) == 0 and status['NoNewPrivs'].strip() == '1' and status['Seccomp'].strip() == '2'
    source = Path('/usr/share/rgg/source.tar.gz')
    binary = Path('/usr/local/bin/rgg-server'); admin = '/usr/local/bin/rgg-admin'
    common = ['--server', str(binary), '--admin', admin]
    checks = [('tools/verify_repository.py', []), ('tests/client_package.py', []), ('tests/legacy_inspection.py', []), ('tests/lifecycle.py', ['--binary', str(binary)])]
    checks += [('tests/' + name + '.py', common) for name in [
        'accounts_cli', 'backups_cli', 'keys_cli', 'password_cli', 'gg6_sessions', 'session_contracts', 'gg6_concurrent_replay', 'gg6_history_retention',
        'gg6_presence_contacts', 'gg6_directory', 'http_discovery', 'discovery_proxy', 'http_registration', 'http_password',
        'http_statistics', 'http_downloads', 'http_patcher', 'http_limits', 'http_metrics', 'runtime_metrics', 'process_metrics', 'r0_smoke', 'gg6_conferences',
        'gg6_images', 'legacy_invitations', 'legacy_import']]
    started = time.monotonic()
    completed = []
    with tempfile.TemporaryDirectory(prefix='rgg-image-source-') as tmp:
        root = Path(tmp)
        with tarfile.open(source, 'r:gz') as archive:
            members = [member for member in archive if member.name.removeprefix('./').split('/')[0] not in ('vendor', '.cargo')]
            archive.extractall(root, members=members, filter='data')
        revision = (root / 'SOURCE-COMMIT').read_text().strip()
        for relative, options in checks:
            subprocess.run([sys.executable, str(root / relative), *options], cwd=root, check=True, timeout=180)
            completed.append(relative)
    report = dict(format=1, passed=True, source_commit=revision,
                  binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                  source_archive_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
                  seconds=round(time.monotonic()-started, 3), completed=completed,
                  uid=10001, memory_max=268435456, cpu_max='100000 100000', pids_max=128,
                  read_only_root=True, no_new_privileges=True, seccomp_filter=True,
                  swap_limit_file_present=(group / 'memory.swap.max').exists(),
                  swap_max=(group / 'memory.swap.max').read_text().strip(),
                  native_client_qualified=False, capacity_r0_qualified=False)
    print('CONTAINER_REPORT=' + json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
