# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Offline checks of source provenance, crate boundaries and license notices (Python >=3.11)."""
from pathlib import Path
import hashlib
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def check(condition, message):
    if not condition:
        raise SystemExit(message)


def local_file(relative):
    path = (ROOT / relative).resolve()
    check(path.is_relative_to(ROOT) and path.is_file(), f"Missing/unsafe file: {relative}")
    return path


def verify():
    manifest = tomllib.loads((ROOT / "THIRD_PARTY.toml").read_text(encoding="utf-8"))
    check(manifest["schema_version"] == 1, "Unsupported provenance schema")
    ids = set()
    for entry in manifest["sources"]:
        check(entry["id"] not in ids, "Duplicate provenance id")
        ids.add(entry["id"])
        for key in ("upstream_url", "upstream_authors", "changes", "verification", "reviewed_on"):
            check(bool(entry.get(key)), f"Missing {key}: {entry['id']}")
        texts = [local_file(p).read_text(encoding="utf-8") for p in entry["local_paths"]]
        for symbol in entry["symbols"]:
            check(any(symbol in text for text in texts), f"Missing mapped symbol {symbol}")
        if entry["kind"] == "reference-only":
            continue
        check(entry["kind"] in ("copied", "adapted", "ported", "test-derived"), "Unknown provenance kind")
        check(re.fullmatch(r"[0-9a-f]{40}", entry["commit"]), "Unpinned source revision")
        check(len(entry["source_paths"]) == len(entry["source_copies"]) == len(entry["source_sha256"]), "Incomplete source mapping")
        for path, expected in zip(entry["source_copies"], entry["source_sha256"]):
            check(hashlib.sha256(local_file(path).read_bytes()).hexdigest() == expected, f"Source hash mismatch: {path}")
        check(hashlib.sha256(local_file(entry["license_path"]).read_bytes()).hexdigest() == entry["license_sha256"], "License hash mismatch")
        for text in texts:
            check(entry["id"] in text, "Missing local provenance marker")
            for author in entry["upstream_authors"]:
                check(author in text, f"Missing author notice: {author}")

    allowed = {
        "rgg-domain": set(),
        "rgg-application": {"rgg-domain"},
        "rgg-protocol": {"rgg-domain", "rgg-application"},
        "rgg-store-sqlite": {"rgg-domain", "rgg-application"},
        "rgg-transport": {"rgg-domain", "rgg-application", "rgg-protocol"},
        "rgg-http": {"rgg-domain", "rgg-application"},
        "rgg-credentials": {"rgg-domain", "rgg-application"},
        "rgg-admin": {"rgg-domain", "rgg-application", "rgg-store-sqlite", "rgg-credentials", "rgg-protocol"},
        "rgg-transfers": {"rgg-domain", "rgg-application"},
        "rgg-server": {"rgg-transfers", "rgg-domain", "rgg-application", "rgg-protocol", "rgg-store-sqlite", "rgg-transport", "rgg-http", "rgg-credentials"},
    }
    root_manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    check(root_manifest["workspace"]["lints"]["rust"]["unsafe_code"] == "forbid", "Own unsafe code must remain forbidden")
    for member in root_manifest["workspace"]["members"]:
        crate = tomllib.loads(local_file(f"{member}/Cargo.toml").read_text(encoding="utf-8"))
        name = crate["package"]["name"]
        check(name in allowed, f"Review architecture before adding {name}")
        check(crate["lints"].get("workspace") is True, f"Missing inherited lints: {name}")
        dependencies = crate.get("dependencies", {})
        check({d for d in dependencies if d.startswith("rgg-")} <= allowed[name], f"Forbidden crate dependency: {name}")
        if name in {"rgg-domain", "rgg-application", "rgg-protocol"}:
            check(not any(d == "axum" or d == "sqlx" or d.startswith("sqlx-") for d in dependencies), f"Infrastructure leaked into {name}")
        if name in {"rgg-domain", "rgg-protocol"}:
            check("tokio" not in dependencies, f"Runtime dependency in pure crate: {name}")
        for source in (ROOT / member / "src").rglob("*.rs"):
            text = source.read_text(encoding="utf-8")
            check("SPDX-License-Identifier: AGPL-3.0-or-later" in text[:500], f"Missing license notice: {source}")
            check("SPDX-FileCopyrightText:" in text[:500], f"Missing author notice: {source}")
    storage = ROOT / "crates/rgg-store-sqlite"
    scripts = {p.name for p in (storage / "migrations").glob("*.sql")}
    embedded = set(re.findall(r'include_str!\(\s*"\.\./migrations/([^\"]+)"\s*\)', (storage / "src/migrations.rs").read_text(encoding="utf-8")))
    check(scripts == embedded, "Every migration must be embedded exactly in the registered source set")
    maximum = max(int(name.split("_", 1)[0]) for name in scripts)
    schema = re.search(r"SCHEMA_VERSION: i64 = (\d+)", (storage / "src/lib.rs").read_text(encoding="utf-8"))
    check(schema is not None and int(schema.group(1)) == maximum, "Schema version does not match embedded migrations")
    print(f"OK: {len(ids)} provenance records, {len(allowed)} crate boundaries, {len(scripts)} migrations, source notices")


if __name__ == "__main__":
    verify()
