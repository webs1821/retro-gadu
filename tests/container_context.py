# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Build contexts contain committed files only, not local secrets/settings."""
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import build_container

class ContextTests(unittest.TestCase):
    def test_source_identity_and_archive_exclude_untracked_files(self):
        with tempfile.TemporaryDirectory(prefix='rgg-container-source-') as tmp:
            root = Path(tmp); repo = root / 'repo'; repo.mkdir()
            git = ['git', '-C', str(repo)]
            subprocess.run(git + ['init', '-q'], check=True)
            (repo / 'Cargo.toml').write_bytes(b'[workspace.package]\nversion = "0.1.0-test"\n')
            subprocess.run(git + ['add', 'Cargo.toml'], check=True)
            subprocess.run(git + ['-c', 'user.name=Synthetic', '-c', 'user.email=synthetic@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-qm', 'synthetic source'], check=True)
            (repo / 'private-secret.txt').write_text('SYNTHETIC-NOT-FOR-BUILD')
            with patch.object(build_container, 'ROOT', repo):
                env = build_container.git_environment(root)
                command, commit, version = build_container.source(env)
                self.assertEqual(version, '0.1.0-test'); self.assertEqual(len(commit), 40)
                archive = root / 'source.tar'
                with archive.open('wb') as stream:
                    subprocess.run(command + ['archive', '--format=tar', commit], stdout=stream, env=env, check=True)
                with tarfile.open(archive) as context:
                    self.assertEqual(context.getnames(), ['Cargo.toml'])
                (repo / 'Cargo.toml').write_text('uncommitted changes')
                with self.assertRaises(ValueError):build_container.source(env)

    def test_regular_file_is_not_a_docker_endpoint(self):
        with tempfile.TemporaryDirectory(prefix='rgg-container-endpoint-') as tmp:
            path = Path(tmp) / 'not-a-socket'; path.write_text('synthetic')
            result = subprocess.run([sys.executable, str(ROOT / 'tools/build_container.py'), '--docker', 'must-not-run', '--socket', str(path), '--report', str(Path(tmp) / 'report.json')], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b'explicit existing local Unix socket', result.stderr)
            self.assertFalse((Path(tmp) / 'report.json').exists())

if __name__ == '__main__':unittest.main(verbosity=2)
