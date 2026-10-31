-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
-- No account reference: retiring an account must not permit reuse of its key/nonce pair.
CREATE TABLE secret_nonces (
    key_id TEXT NOT NULL,
    nonce BLOB NOT NULL CHECK(length(nonce) = 24),
    PRIMARY KEY(key_id,nonce)
) WITHOUT ROWID;
INSERT INTO secret_nonces(key_id,nonce) SELECT key_id,nonce FROM accounts;
