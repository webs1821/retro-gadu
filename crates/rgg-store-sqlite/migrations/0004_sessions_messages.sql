-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
CREATE TABLE account_sessions (
    uin INTEGER PRIMARY KEY REFERENCES accounts(uin) ON DELETE CASCADE,
    session_id BLOB NOT NULL UNIQUE CHECK(length(session_id)=16),
    epoch TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK(generation > 0),
    credential_version INTEGER NOT NULL CHECK(credential_version > 0),
    ready INTEGER NOT NULL DEFAULT 0 CHECK(ready IN (0,1))
);
CREATE TABLE account_privacy (
    uin INTEGER PRIMARY KEY REFERENCES accounts(uin) ON DELETE CASCADE,
    revision INTEGER NOT NULL DEFAULT 0,
    friends_only INTEGER NOT NULL DEFAULT 0 CHECK(friends_only IN (0,1))
);
CREATE TABLE contact_rules (
    owner INTEGER NOT NULL REFERENCES accounts(uin) ON DELETE CASCADE,
    peer INTEGER NOT NULL CHECK(peer BETWEEN 1 AND 4294967295),
    flags INTEGER NOT NULL CHECK(flags BETWEEN 0 AND 7),
    PRIMARY KEY(owner,peer)
) WITHOUT ROWID;
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sender INTEGER NOT NULL,
    recipient INTEGER NOT NULL,
    client_sequence INTEGER NOT NULL CHECK(client_sequence BETWEEN 0 AND 4294967295),
    payload_version INTEGER NOT NULL CHECK(payload_version=1),
    presentation INTEGER NOT NULL CHECK(presentation IN (0,4,8,12)),
    body TEXT,
    styles TEXT,
    payload_bytes INTEGER NOT NULL CHECK(payload_bytes>0),
    accepted_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    target_session BLOB
);
CREATE TABLE deliveries (
    message_id INTEGER PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    recipient INTEGER NOT NULL,
    enqueue_order INTEGER NOT NULL UNIQUE,
    state TEXT NOT NULL CHECK(state IN ('Pending','Leased','TransportWrittenUnconfirmed','Expired','RejectedByPrivacy','Rejected')),
    lease_token BLOB,
    session_id BLOB,
    epoch TEXT,
    generation INTEGER,
    lease_until INTEGER,
    terminal_at INTEGER,
    reason TEXT,
    CHECK((state='Leased' AND lease_token IS NOT NULL AND length(lease_token)=16 AND session_id IS NOT NULL AND length(session_id)=16 AND epoch IS NOT NULL AND generation IS NOT NULL AND generation>0 AND lease_until IS NOT NULL)
        OR (state!='Leased' AND lease_token IS NULL AND session_id IS NULL AND epoch IS NULL AND generation IS NULL AND lease_until IS NULL)),
    CHECK((state IN ('Pending','Leased') AND terminal_at IS NULL) OR (state NOT IN ('Pending','Leased') AND terminal_at IS NOT NULL))
);
CREATE INDEX deliveries_fifo ON deliveries(recipient,enqueue_order) WHERE state IN ('Pending','Leased');
CREATE INDEX messages_sender ON messages(sender);
CREATE INDEX messages_expiry ON messages(expires_at);
CREATE INDEX deliveries_terminal ON deliveries(terminal_at) WHERE terminal_at IS NOT NULL;
CREATE TABLE delivery_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL REFERENCES deliveries(message_id) ON DELETE CASCADE,
    token BLOB NOT NULL UNIQUE CHECK(length(token)=16),
    session_id BLOB NOT NULL,
    epoch TEXT NOT NULL,
    generation INTEGER NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    outcome TEXT CHECK(outcome IN ('TransportWrittenUnconfirmed','Interrupted','RecoveredEpoch'))
);
CREATE INDEX delivery_attempts_message ON delivery_attempts(message_id,id DESC);
