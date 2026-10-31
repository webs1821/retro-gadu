# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Pipe `cargo metadata --locked --format-version 1` here; metadata is data, never executed."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import tomllib


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    metadata = json.load(sys.stdin)
    lock = (root / "Cargo.lock").read_bytes()
    locked = {(p["name"], p["version"]): p for p in tomllib.loads(lock.decode())["package"]}
    members = set(metadata["workspace_members"])
    packages = []
    for package in metadata["packages"]:
        if package["id"] in members:
            continue
        info = locked[(package["name"], package["version"])]
        packages.append({"name": package["name"], "version": package["version"],
                         "license": package["license"], "repository": package["repository"],
                         "source": info.get("source"), "checksum": info.get("checksum")})
    report = {"schema_version": 1, "cargo_lock_sha256": hashlib.sha256(lock).hexdigest(),
              "packages": sorted(packages, key=lambda p: (p["name"], p["version"]))}
    destination = root / "docs/implementation/dependencies.json"
    if args.write:
        with destination.open("w", encoding="utf-8", newline="\n") as handle:
            json.dump(report, handle, indent=2, ensure_ascii=False)
            handle.write("\n")
    else:
        assert json.loads(destination.read_text(encoding="utf-8")) == report, "Dependency inventory does not match Cargo.lock/metadata"
    print(f"OK: {len(packages)} dependency versions inventoried")


if __name__ == "__main__":
    main()
