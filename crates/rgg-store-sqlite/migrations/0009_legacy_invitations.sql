-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
-- A separate namespace retains exact legacy SHA-256 hashes, quotas and revocations.
CREATE TABLE legacy_invitations (
    digest BLOB PRIMARY KEY NOT NULL CHECK(length(digest)=32),
    max_uses INTEGER NOT NULL CHECK(max_uses=3),
    used INTEGER NOT NULL CHECK(used BETWEEN 0 AND max_uses),
    active INTEGER NOT NULL CHECK(active IN (0,1)),
    created_at INTEGER NOT NULL
);
