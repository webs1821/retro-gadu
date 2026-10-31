# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Offline administrator E2E checks using synthetic accounts and an isolated temporary database."""
import argparse
import json
import re
from pathlib import Path
import sqlite3
import subprocess
import tempfile
from lifecycle import Server


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--admin", type=Path, required=True)
    parser.add_argument("--server", type=Path, required=True)
    args = parser.parse_args()
    admin, server_binary = args.admin.resolve(strict=True), args.server.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="rgg-accounts-") as temporary:
        root = Path(temporary)
        data = root / "data"

        def run(command, *extra, input_value=None, success=True):
            result = subprocess.run([str(admin), command, "--data-dir", str(data), *extra],
                                    input=json.dumps(input_value).encode() if input_value is not None else None,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=12)
            assert b"LocalTest9!" not in result.stdout + result.stderr
            if success:
                assert result.returncode == 0, result.stderr.decode()
                return json.loads(result.stdout)
            assert result.returncode != 0
            return result

        assert run("init") == {"initialized": True}
        invitation = run("invite")["codes"][0]

        def register(number=None, code=invitation):
            return {"requested_uin": number, "invite": code, "password": "LocalTest9!", "email": "test@example.invalid"}

        assert run("account-create", input_value=register(742858)) == {"uin": 742858}
        run("account-create", input_value=register(742858), success=False)
        assert run("account-create", input_value=register()) == {"uin": 112233}
        assert run("account-create", input_value=register()) == {"uin": 112234}
        run("account-create", input_value=register(), success=False)
        assert run("stats") == {"registered": 3}
        run("retire", "--uin", "742858")
        assert run("stats") == {"registered": 2}
        second_code = run("invite")["codes"][0]
        run("account-create", input_value=register(742858, second_code), success=False)
        assert run("account-create", input_value=register(12, second_code)) == {"uin": 12}

        with sqlite3.connect(str(data / "rgg.sqlite")) as db:
            assert db.execute("SELECT used FROM invitations ORDER BY created_at, rowid").fetchall() == [(3,), (1,)]
            assert db.execute("SELECT count(*) FROM retired_uins WHERE uin=742858").fetchone()[0] == 1
            assert db.execute("SELECT email_verified FROM accounts WHERE uin=12").fetchone()[0] == 0
            assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
        for file in data.glob("rgg.sqlite*"):
            content = file.read_bytes()
            assert b"LocalTest9!" not in content
            assert invitation.encode() not in content

        keyfile = data / "keys.toml"
        original_key = keyfile.read_bytes()
        keyfile.rename(data / "keys.saved")
        result = run("init", success=False)
        assert b"original keyring" in result.stderr and not keyfile.exists()
        (data / "keys.saved").rename(keyfile)
        assert keyfile.read_bytes() == original_key
        assert run("init") == {"initialized": True}
        wrong_key = re.sub(rb'"[0-9a-f]{64}"', b'"' + b'00' * 32 + b'"', original_key)
        assert wrong_key != original_key
        keyfile.write_bytes(wrong_key)
        result = run("init", success=False)
        assert b"envelope failed verification" in result.stderr
        keyfile.write_bytes(original_key)
        assert run("init") == {"initialized": True}

        config = root / "rgg.toml"
        config.write_text("data_dir='data'\nhttp_bind='127.0.0.1:0'\n", encoding="utf-8")
        server = Server(server_binary, config, root / "server.log")
        try:
            server.ready()
            result = run("stats", success=False)
            assert b"another process owns" in result.stderr
        finally:
            server.close()
        assert run("stats") == {"registered": 3}
        print("PASS: CLI init/register/invites, three-use limit, custom/automatic UIN, persistent retirement, encrypted credentials, lost-key refusal, server/admin mutual exclusion")


if __name__ == "__main__":
    main()
