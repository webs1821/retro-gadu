// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! SQL is embedded in the binary, while SQLx's public migrator validates versions and checksums.
//! Avoid macro-only dependencies on database drivers that rGG never uses.
use crate::database as sqlx;
use futures_util::future::BoxFuture;
use sqlx::{
    error::BoxDynError,
    migrate::{Migration, MigrationSource, MigrationType},
};

#[derive(Debug)]
pub(crate) struct EmbeddedMigrations;

impl MigrationSource<'static> for EmbeddedMigrations {
    fn resolve(self) -> BoxFuture<'static, Result<Vec<Migration>, BoxDynError>> {
        Box::pin(async {
            Ok(vec![
                Migration::new(
                    1,
                    "runtime".into(),
                    MigrationType::Simple,
                    include_str!("../migrations/0001_runtime.sql").into(),
                    false,
                ),
                Migration::new(
                    2,
                    "accounts".into(),
                    MigrationType::Simple,
                    include_str!("../migrations/0002_accounts.sql").into(),
                    false,
                ),
                Migration::new(
                    3,
                    "secret nonces".into(),
                    MigrationType::Simple,
                    include_str!("../migrations/0003_secret_nonces.sql").into(),
                    false,
                ),
                Migration::new(
                    4,
                    "sessions messages".into(),
                    MigrationType::Simple,
                    include_str!("../migrations/0004_sessions_messages.sql").into(),
                    false,
                ),
            ])
        })
    }
}
