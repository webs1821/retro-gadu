-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
-- Ordered JSON UIN array; [] upgrades existing text/style payloads without inventing recipients.
ALTER TABLE messages ADD COLUMN conference TEXT DEFAULT '[]'
    CHECK(conference IS NULL OR length(CAST(conference AS BLOB))<=512);
UPDATE messages SET payload_bytes=payload_bytes+2 WHERE body IS NOT NULL;
UPDATE messages SET conference=NULL WHERE body IS NULL;
