-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
CREATE TABLE admin_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    actor TEXT NOT NULL CHECK(actor='local_cli_with_current_password'),
    action TEXT NOT NULL CHECK(action='password_changed'),
    target_uin INTEGER NOT NULL CHECK(target_uin BETWEEN 1 AND 4294967295),
    previous_version INTEGER NOT NULL CHECK(previous_version>0),
    credential_version INTEGER NOT NULL CHECK(credential_version=previous_version+1),
    occurred_at INTEGER NOT NULL
);
CREATE INDEX admin_audit_time ON admin_audit(occurred_at);
