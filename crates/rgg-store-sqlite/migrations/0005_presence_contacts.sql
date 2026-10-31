-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
CREATE TABLE account_presence (
    uin INTEGER PRIMARY KEY REFERENCES accounts(uin) ON DELETE CASCADE,
    availability TEXT NOT NULL CHECK(availability IN ('offline','available','away','invisible')),
    description TEXT CHECK(length(CAST(description AS BLOB))<=210),
    return_at INTEGER CHECK(return_at BETWEEN 0 AND 4294967295),
    session_id BLOB CHECK(session_id IS NULL OR length(session_id)=16),
    CHECK(return_at IS NULL OR description IS NOT NULL),
    CHECK(availability='offline' OR session_id IS NOT NULL)
);
-- The archive is an independent, versioned UTF-8 document, never the active privacy list.
CREATE TABLE contact_archives (
    owner INTEGER PRIMARY KEY REFERENCES accounts(uin) ON DELETE CASCADE,
    version INTEGER NOT NULL CHECK(version>0),
    format INTEGER NOT NULL CHECK(format=1),
    document TEXT NOT NULL CHECK(length(CAST(document AS BLOB))<=786432)
);
CREATE TABLE contact_staging (
    owner INTEGER PRIMARY KEY REFERENCES accounts(uin) ON DELETE CASCADE,
    session_id BLOB NOT NULL CHECK(length(session_id)=16),
    epoch TEXT NOT NULL,
    generation INTEGER NOT NULL,
    base_version INTEGER NOT NULL CHECK(base_version>=0),
    document TEXT NOT NULL CHECK(length(CAST(document AS BLOB))<=786432)
);
