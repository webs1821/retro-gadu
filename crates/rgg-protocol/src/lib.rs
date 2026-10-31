// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821

//! Pure wire adapters. They perform no I/O and cannot access the database.

pub mod frame;
pub mod gg6;
pub mod gg6_contacts;
pub mod gg6_messages;
pub mod gg6_presence;
pub mod legacy_hash;
