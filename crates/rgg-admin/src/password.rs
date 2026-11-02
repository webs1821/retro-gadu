// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::Error;
use rgg_application::accounts::LegacyPassword;
use rgg_credentials::Keyring;
use rgg_domain::UserId;
use rgg_store_sqlite::SqliteStore;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use zeroize::Zeroizing;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    current_password: Zeroizing<String>,
    new_password: Zeroizing<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetInput {
    confirm_uin: u32,
    new_password: Zeroizing<String>,
}
pub async fn reset(
    store: &SqliteStore,
    user: UserId,
    backup: &std::path::Path,
) -> Result<Value, Error> {
    if store
        .data_directory()
        .join("keys.pending")
        .symlink_metadata()
        .is_ok()
    {
        return Err("resume pending key rotation before resetting passwords".into());
    }
    let mut raw = Zeroizing::new(Vec::new());
    std::io::stdin().take(4097).read_to_end(&mut raw)?;
    if raw.len() > 4096 {
        return Err("password input exceeds limit".into());
    }
    let input: ResetInput =
        serde_json::from_slice(&raw).map_err(|_| "invalid password reset JSON")?;
    drop(raw);
    if input.confirm_uin != user.get() {
        return Err("confirmed UIN does not match target".into());
    }
    let new = LegacyPassword::new(Zeroizing::new(input.new_password.as_bytes().to_vec()))?;
    drop(input);
    let ring = Keyring::load(&store.data_directory().join("keys.toml"))?;
    // Publication and verification of the complete snapshot must finish before the transaction.
    crate::backup::create(store, backup).await?;
    let version = store.reset_password_offline(user, &new, &ring).await?;
    Ok(
        json!({"password_reset":true,"uin":user.get(),"credential_version":version,"backup_created":true}),
    )
}
pub async fn change(store: &SqliteStore, user: UserId) -> Result<Value, Error> {
    if store
        .data_directory()
        .join("keys.pending")
        .symlink_metadata()
        .is_ok()
    {
        return Err("resume pending key rotation before changing passwords".into());
    }
    let mut raw = Zeroizing::new(Vec::new());
    std::io::stdin().take(4097).read_to_end(&mut raw)?;
    if raw.len() > 4096 {
        return Err("password input exceeds limit".into());
    }
    let input: Input = serde_json::from_slice(&raw).map_err(|_| "invalid password change JSON")?;
    drop(raw);
    let current = LegacyPassword::new(Zeroizing::new(input.current_password.as_bytes().to_vec()))?;
    let new = LegacyPassword::new(Zeroizing::new(input.new_password.as_bytes().to_vec()))?;
    drop(input);
    let ring = Keyring::load(&store.data_directory().join("keys.toml"))?;
    let version = store
        .change_password_offline(user, &current, &new, &ring)
        .await?;
    Ok(json!({"password_changed":true,"uin":user.get(),"credential_version":version}))
}
pub async fn audit(store: &SqliteStore) -> Result<Value, Error> {
    let rows = store.password_audit().await?;
    let events:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.id,"actor":r.actor,"action":r.action,"uin":r.user,"previous_version":r.previous_version,"credential_version":r.credential_version,"occurred_at":r.occurred_at})).collect();
    Ok(json!({"events":events,"limit":100,"window_days":30}))
}
