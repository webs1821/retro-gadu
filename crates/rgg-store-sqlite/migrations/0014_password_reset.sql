-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
CREATE TABLE admin_audit_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    target_uin INTEGER NOT NULL CHECK(target_uin BETWEEN 1 AND 4294967295),
    previous_version INTEGER NOT NULL CHECK(previous_version>0),
    credential_version INTEGER NOT NULL CHECK(credential_version=previous_version+1),
    occurred_at INTEGER NOT NULL,
    CHECK((action='password_changed' AND actor IN ('local_cli_with_current_password','local_web_with_current_password'))
       OR (action='password_reset' AND actor='local_cli_recovery'))
);
INSERT INTO admin_audit_next SELECT * FROM admin_audit;
-- Retention may have removed every row; do not reuse historical audit IDs.
INSERT INTO sqlite_sequence(name,seq)
    SELECT 'admin_audit_next',seq FROM sqlite_sequence WHERE name='admin_audit'
    AND NOT EXISTS(SELECT 1 FROM sqlite_sequence WHERE name='admin_audit_next');
UPDATE sqlite_sequence SET seq=max(seq,coalesce((SELECT seq FROM sqlite_sequence WHERE name='admin_audit'),0))
    WHERE name='admin_audit_next';
DROP TABLE admin_audit;
ALTER TABLE admin_audit_next RENAME TO admin_audit;
CREATE INDEX admin_audit_time ON admin_audit(occurred_at);
