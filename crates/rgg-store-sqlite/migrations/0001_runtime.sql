-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
CREATE TABLE runtime_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    process_epoch TEXT NOT NULL,
    started_at INTEGER NOT NULL
);
