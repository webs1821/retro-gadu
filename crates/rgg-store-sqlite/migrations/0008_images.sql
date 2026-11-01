-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
ALTER TABLE account_sessions ADD COLUMN image_limit_kib INTEGER NOT NULL DEFAULT 0 CHECK(image_limit_kib BETWEEN 0 AND 255);
-- Deliberately no FK to messages: purging text/history must not revoke a delivered offer.
CREATE TABLE image_offers (
    message_id INTEGER NOT NULL,
    sender INTEGER NOT NULL REFERENCES accounts(uin) ON DELETE CASCADE,
    recipient INTEGER NOT NULL REFERENCES accounts(uin) ON DELETE CASCADE,
    size INTEGER NOT NULL CHECK(size BETWEEN 1 AND 261120),
    crc32 INTEGER NOT NULL CHECK(crc32 BETWEEN 0 AND 4294967295),
    expires_at INTEGER NOT NULL,
    PRIMARY KEY(message_id,size,crc32)
) WITHOUT ROWID;
CREATE INDEX image_offers_authorize ON image_offers(recipient,sender,size,crc32,expires_at);
CREATE INDEX image_offers_expiry ON image_offers(expires_at);
CREATE INDEX image_offers_sender ON image_offers(sender);
