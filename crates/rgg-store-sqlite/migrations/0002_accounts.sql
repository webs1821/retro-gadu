-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
CREATE TABLE accounts (
    uin INTEGER PRIMARY KEY CHECK(uin BETWEEN 1 AND 4294967295),
    active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1)),
    email TEXT NOT NULL,
    email_verified INTEGER NOT NULL DEFAULT 0 CHECK(email_verified IN (0,1)),
    credential_version INTEGER NOT NULL DEFAULT 1 CHECK(credential_version > 0),
    session_generation INTEGER NOT NULL DEFAULT 0 CHECK(session_generation >= 0),
    secret_version INTEGER NOT NULL CHECK(secret_version = 1),
    key_id TEXT NOT NULL,
    nonce BLOB NOT NULL CHECK(length(nonce) = 24),
    ciphertext BLOB NOT NULL CHECK(length(ciphertext) BETWEEN 24 AND 31),
    created_at INTEGER NOT NULL
);
CREATE TABLE retired_uins (
    uin INTEGER PRIMARY KEY CHECK(uin BETWEEN 1 AND 4294967295),
    retired_at INTEGER NOT NULL
);
CREATE TABLE invitations (
    digest BLOB PRIMARY KEY CHECK(length(digest) = 32),
    max_uses INTEGER NOT NULL CHECK(max_uses BETWEEN 1 AND 1000),
    used INTEGER NOT NULL DEFAULT 0 CHECK(used BETWEEN 0 AND max_uses),
    created_at INTEGER NOT NULL
);
CREATE TABLE number_allocator (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    next_uin INTEGER NOT NULL CHECK(next_uin >= 112233)
);
INSERT INTO number_allocator(id,next_uin) VALUES(1,112233);
