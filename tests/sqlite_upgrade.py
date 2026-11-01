# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Upgrade a snapshot produced by actual older binaries; only synthetic local data.

The old admin restores its own snapshot before the new runtime opens it. No
baseline is fabricated by editing migration versions/checksums in a new database.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import tempfile

from backups_cli import create, hashes, prepare, restore, verify
from gg6_directory import query
from gg6_images import download, text
from gg6_presence_contacts import barrier, get_archive, rules
from gg6_sessions import Lab, until


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def message(client, expected):
    kind, body = client.receive()
    assert kind == 10 and body[16:] == expected.encode("cp1250") + b"\0"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", type=Path, required=True)
    parser.add_argument("--admin", type=Path, required=True)
    parser.add_argument("--baseline-dir", type=Path, required=True)
    args = parser.parse_args()
    assert os.name == "posix", "this qualification currently requires POSIX"
    server, admin = args.server.resolve(), args.admin.resolve()
    old_server = args.baseline_dir.resolve() / "rgg-server"
    old_admin = args.baseline_dir.resolve() / "rgg-admin"
    binaries = {"baseline_server": sha(old_server), "baseline_admin": sha(old_admin),
                "server": sha(server), "admin": sha(admin)}
    assert binaries["baseline_server"] != binaries["server"], "use actual baseline binaries"
    assert binaries["baseline_admin"] != binaries["admin"], "use actual baseline binaries"
    with tempfile.TemporaryDirectory(prefix="rgg-sqlite-upgrade-") as temporary:
        root = Path(temporary)
        lab = Lab(old_server, old_admin, root)
        try:
            prepare(lab)  # profile, contacts/privacy, retired UIN and pending image offer
            lab.start()
            sender = lab.client(1, ready=False)
            rules(sender, [(3, 4)])
            barrier(sender)
            assert sender.message(2, 778, "przed aktualizacją") == 3
            lab.stop()
            backup = root / "old-snapshot"
            create(lab, backup)
            assert verify(lab, backup)["verified"]
            frozen = hashes(backup)
            migrations = lab.sql("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
            source = lab.data
            source_hashes = {name: sha(source / name) for name in ("rgg.sqlite", "keys.toml")}
            restored = root / "restored"
            assert restore(lab, backup, restored)["restored"]

            lab.binary, lab.admin, lab.data = server, admin, restored
            runtime = root / "new-runtime"
            runtime.mkdir()
            (runtime / "data").symlink_to(restored, target_is_directory=True)
            lab.root = runtime
            assert lab.run("stats")["registered"] == 2
            assert lab.sql("SELECT uin FROM retired_uins") == [(3,)]
            assert lab.sql("SELECT peer,flags FROM contact_rules WHERE owner=1") == [(3, 4)]
            assert lab.sql("SELECT used FROM invitations") == [(3,)]
            assert lab.sql("SELECT version,checksum FROM _sqlx_migrations ORDER BY version") == migrations
            lab.start()
            recipient = lab.client(2)
            text(recipient)
            message(recipient, "przed aktualizacją")
            sender = lab.client(1)
            barrier(sender)
            assert query(sender, 2)["firstname"] == "Próba kopii"
            assert get_archive(sender) == "Znajomy;2\r\n".encode("cp1250")
            download(sender, recipient)
            assert sender.message(2, 779, "po aktualizacji") == 3
            message(recipient, "po aktualizacji")
            until(lambda: lab.sql("SELECT count(*) FROM deliveries WHERE state!='TransportWrittenUnconfirmed'") == [(0,)])
            lab.stop()
            assert lab.sql("PRAGMA integrity_check") == [("ok",)]
            assert lab.sql("PRAGMA foreign_key_check") == []
            assert lab.sql("SELECT count(*) FROM account_sessions") == [(0,)]
            assert hashes(backup) == frozen
            assert {name: sha(source / name) for name in source_hashes} == source_hashes
            assert (restored / "keys.toml").read_bytes() == (backup / "keys.toml").read_bytes()
            current_backup = root / "new-snapshot"
            create(lab, current_backup)
            assert verify(lab, current_backup)["verified"]
            assert restore(lab, current_backup, root / "new-restored")["restored"]
            print(json.dumps({"passed": True, "binaries_sha256": binaries,
                              "migration_checksums_preserved": len(migrations),
                              "old_snapshot_and_source_unchanged": True,
                              "old_credentials_offline_text_image_contacts_profile": True,
                              "new_write_and_backup_restore": True}, indent=2))
        finally:
            lab.stop()


if __name__ == "__main__":
    main()
