-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
CREATE TABLE import_receipt (
    id INTEGER PRIMARY KEY CHECK(id=1),
    source_sha256 TEXT NOT NULL CHECK(length(source_sha256)=64),
    package_sha256 TEXT NOT NULL CHECK(length(package_sha256)=64),
    imported_at INTEGER NOT NULL,
    summary TEXT NOT NULL CHECK(length(CAST(summary AS BLOB))<=8192)
);
-- Source account names are private metadata, never an implicit directory profile.
CREATE TABLE imported_account_metadata (
    uin INTEGER PRIMARY KEY REFERENCES accounts(uin) ON DELETE CASCADE,
    source_name TEXT NOT NULL CHECK(length(CAST(source_name AS BLOB))<=1024),
    source_created_at INTEGER,
    invite_digest BLOB REFERENCES legacy_invitations(digest)
);
