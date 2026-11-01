-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
CREATE TABLE admin_audit_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    actor TEXT NOT NULL CHECK(actor IN ('local_cli_with_current_password','local_web_with_current_password')),
    action TEXT NOT NULL CHECK(action='password_changed'),
    target_uin INTEGER NOT NULL CHECK(target_uin BETWEEN 1 AND 4294967295),
    previous_version INTEGER NOT NULL CHECK(previous_version>0),
    credential_version INTEGER NOT NULL CHECK(credential_version=previous_version+1),
    occurred_at INTEGER NOT NULL
);
INSERT INTO admin_audit_next SELECT * FROM admin_audit;
-- Preserve the high-water mark even when retention removed the highest/all records.
INSERT INTO sqlite_sequence(name,seq)
    SELECT 'admin_audit_next',seq FROM sqlite_sequence WHERE name='admin_audit'
    AND NOT EXISTS(SELECT 1 FROM sqlite_sequence WHERE name='admin_audit_next');
UPDATE sqlite_sequence SET seq=max(seq,coalesce((SELECT seq FROM sqlite_sequence WHERE name='admin_audit'),0))
    WHERE name='admin_audit_next';
DROP TABLE admin_audit;
ALTER TABLE admin_audit_next RENAME TO admin_audit;
CREATE INDEX admin_audit_time ON admin_audit(occurred_at);
CREATE TABLE password_change_budget (
    uin INTEGER PRIMARY KEY REFERENCES accounts(uin) ON DELETE CASCADE,
    window_started INTEGER NOT NULL,
    attempts INTEGER NOT NULL CHECK(attempts BETWEEN 1 AND 5)
);
