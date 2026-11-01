// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
mod validate;
mod wire;
use crate::{Error, backup};
use rgg_application::accounts::{AccountRepository, SecretProtector};
use rgg_credentials::{Keyring, gg_retro::LegacyKey};
use rgg_store_sqlite::{SqliteStore, import_data};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;
pub async fn run(key_file: &Path, target: Option<&Path>) -> Result<Value, Error> {
    let mut input = Zeroizing::new(Vec::new());
    std::io::stdin()
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut input)?;
    if input.len() > 64 * 1024 * 1024 {
        return Err("import package exceeds 64 MiB".into());
    }
    let package: wire::Package =
        serde_json::from_slice(&input).map_err(|_| "invalid import package JSON")?;
    let digest = hex::encode(Sha256::digest(&*input));
    drop(input);
    let old_key = LegacyKey::load(key_file)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs()
        .try_into()?;
    let (mut data, accounts) = validate::prepare(package, &old_key, digest, now)?;
    let Some(target) = target else {
        return Ok(
            json!({"checked":true,"target_schema":rgg_store_sqlite::SCHEMA_VERSION,"source_sha256":data.source_sha256,"summary":data.summary}),
        );
    };
    backup::private_dir_new(target)?;
    backup::private_new(&target.join("import-incomplete"))?.sync_all()?;
    backup::sync_dir(target)?;
    #[cfg(feature = "test-hooks")]
    if std::env::var("RGG_TEST_IMPORT_CRASH").as_deref() == Ok("before_import_database") {
        std::process::exit(86);
    }
    let store = SqliteStore::open_new_import(target).await?;
    let result = apply(&store, &old_key, &mut data, accounts).await;
    store.close().await;
    result
}
async fn apply(
    store: &SqliteStore,
    old_key: &LegacyKey,
    data: &mut import_data::Dataset,
    accounts: Vec<wire::Account>,
) -> Result<Value, Error> {
    let root = store.data_directory();
    let marker = root.join("import-incomplete");
    let ring = Keyring::create_new(&root.join("keys.toml"))?;
    for a in accounts {
        let user = validate::user(a.uin)?;
        let password = old_key.open(user, &a.password)?;
        let secret = ring.seal(user, &password)?;
        drop(password);
        data.accounts.push(import_data::Account {
            user,
            name: a.name,
            email: a.email,
            created: a.created_at,
            secret,
            invitation: a
                .registration_invite_hash
                .as_deref()
                .map(validate::hash)
                .transpose()?,
        });
    }
    store.import_dataset(data).await?;
    let counts = store.inspect_credentials(&ring).await?;
    if counts.values().sum::<u64>() != data.accounts.len() as u64
        || store.registered_count().await? != data.accounts.len() as u64
    {
        return Err("imported account count mismatch".into());
    }
    #[cfg(feature = "test-hooks")]
    if std::env::var("RGG_TEST_IMPORT_CRASH").as_deref() == Ok("after_import_commit") {
        std::process::exit(86);
    }
    std::fs::remove_file(marker)?;
    backup::sync_dir(root)?;
    if let Some(parent) = root.parent() {
        backup::sync_dir(parent)?;
    }
    Ok(
        json!({"imported":true,"schema":rgg_store_sqlite::SCHEMA_VERSION,"source_sha256":data.source_sha256,"package_sha256":data.package_sha256,"summary":data.summary}),
    )
}
