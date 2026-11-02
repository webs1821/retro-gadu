# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Black-box lifecycle tests. Synthetic databases only; all network traffic is loopback."""
import argparse
import contextlib
import json
import os
from pathlib import Path
import re
import signal
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


class Server:
    def __init__(self, binary, config, log, env=None):
        self.log = log
        self.handle = log.open("wb")
        self.process = subprocess.Popen([str(binary), "--config", str(config)], stdout=self.handle, stderr=self.handle, env=env)
        self.address = None

    def ready(self):
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            assert self.process.poll() is None, self.log.read_text()
            match = re.search(r"address=(127\.0\.0\.1:\d+)", self.log.read_text())
            if match:
                self.address = match.group(1)
                try:
                    with urllib.request.urlopen("http://" + self.address + "/health/ready", timeout=0.3) as response:
                        assert response.status == 200
                        assert json.load(response) == {"service": "rGG", "status": "ready"}
                        return
                except (OSError, urllib.error.HTTPError):
                    pass
            time.sleep(0.02)
        raise AssertionError("Server did not become ready: " + self.log.read_text())

    def close(self, hard=False):
        if self.process.poll() is None:
            if hard:
                self.process.kill()
            else:
                self.process.send_signal(signal.SIGTERM)
            self.process.wait(timeout=10)
        self.handle.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    assert os.name == "posix", "Signal/crash lifecycle suite currently runs on Linux/WSL"
    with tempfile.TemporaryDirectory(prefix="rgg-lifecycle-") as temporary:
        root = Path(temporary)
        config = root / "rgg.toml"
        config.write_text("data_dir='data'\nhttp_bind='127.0.0.1:0'\nshutdown_seconds=1\n", encoding="utf-8")
        with contextlib.ExitStack() as cleanup:
            server = Server(binary, config, root / "first.log")
            cleanup.callback(server.close)
            server.ready()
            with urllib.request.urlopen("http://" + server.address + "/health/live", timeout=2) as response:
                assert json.load(response)["status"] == "live"
            other = subprocess.run([str(binary), "--config", str(config)], capture_output=True, timeout=10)
            assert other.returncode != 0 and b"another process owns" in other.stderr, other.stderr
            with sqlite3.connect(str(root / "data/rgg.sqlite")) as db:
                first_epoch = db.execute("SELECT process_epoch FROM runtime_state").fetchone()[0]
                assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            server.close(hard=True)
            restarted = Server(binary, config, root / "restart.log")
            cleanup.callback(restarted.close)
            restarted.ready()
            with sqlite3.connect(str(root / "data/rgg.sqlite")) as db:
                assert db.execute("SELECT process_epoch FROM runtime_state").fetchone()[0] != first_epoch
                assert db.execute("SELECT max(version) FROM _sqlx_migrations").fetchone()[0] == 14
                assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
            restarted.close()
            assert restarted.process.returncode == 0, restarted.log.read_text()
            assert "shutdown complete" in restarted.log.read_text()
            public = root / "public.toml"
            public.write_text("data_dir='must-not-exist'\nhttp_bind='0.0.0.0:8088'\n", encoding="utf-8")
            refused = subprocess.run([str(binary), "--config", str(public)], capture_output=True, timeout=10)
            assert refused.returncode != 0 and b"loopback" in refused.stderr
            assert not (root / "must-not-exist").exists()
            print("PASS: live/ready HTTP, exclusive process lock, crash/restart, fresh epoch, migration persistence, integrity, graceful SIGTERM, public-bind rejection")


if __name__ == "__main__":
    main()
