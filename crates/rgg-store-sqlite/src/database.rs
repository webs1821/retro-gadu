// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Only SQLite's components are linked. The all-driver facade also resolves unused MySQL/RSA
//! packages into Cargo.lock; explicit, version-matched components avoid that dependency.
pub(crate) use sqlx_core::{
    connection::ConnectOptions,
    error::{self, Error},
    migrate,
    query::query,
    query_scalar::query_scalar,
    row::Row,
    transaction::Transaction,
};
pub(crate) use sqlx_sqlite::{self as sqlite, Sqlite, SqlitePool};
