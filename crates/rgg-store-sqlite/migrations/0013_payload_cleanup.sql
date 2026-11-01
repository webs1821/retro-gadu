-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
-- Cleanup visits retained payloads, not every terminal history row.
CREATE INDEX messages_with_payload ON messages(id)
WHERE body IS NOT NULL OR styles IS NOT NULL OR conference IS NOT NULL;
