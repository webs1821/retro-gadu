// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
mod backup;
use rgg_application::accounts::{AccountRepository, LegacyPassword, Registration, SecretProtector};
use rgg_credentials::{Keyring, invitation_digest, new_invitation};
use rgg_domain::UserId;
use rgg_store_sqlite::SqliteStore;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, io::Read, path::PathBuf};
use zeroize::Zeroizing;
type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountInput {
    requested_uin: Option<u32>,
    invite: Zeroizing<String>,
    password: Zeroizing<String>,
    email: String,
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(value) => {
            println!("{value}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("rgg-admin: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<Value, Error> {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .ok_or("use: rgg-admin <init|invite|account-create|retire|stats|backup-create|backup-verify|restore> [options]")?;
    let allowed: &[&str] = match command.as_str() {
        "init" | "account-create" | "stats" => &["--data-dir"],
        "invite" => &["--data-dir", "--count", "--uses"],
        "retire" => &["--data-dir", "--uin"],
        "backup-create" => &["--data-dir", "--output"],
        "backup-verify" => &["--backup"],
        "restore" => &["--backup", "--data-dir"],
        _ => return Err("unknown command".into()),
    };
    let mut options = BTreeMap::new();
    while let Some(name) = args.next() {
        if !allowed.contains(&name.as_str()) {
            return Err("unknown option".into());
        }
        let value = args.next().ok_or("option requires a value")?;
        if options.insert(name, value).is_some() {
            return Err("duplicate option".into());
        }
    }
    if command == "backup-verify" {
        return backup::verify(std::path::Path::new(
            options.get("--backup").ok_or("--backup is required")?,
        ))
        .await;
    }
    let path = PathBuf::from(options.get("--data-dir").ok_or("--data-dir is required")?);
    if command == "backup-create" {
        if !options.contains_key("--output") {
            return Err("--output is required".into());
        }
        if !path.join("rgg.sqlite").is_file() || !path.join("keys.toml").is_file() {
            return Err("backup source must be an initialized data directory".into());
        }
    }
    if command == "restore" {
        return backup::restore(
            std::path::Path::new(options.get("--backup").ok_or("--backup is required")?),
            &path,
        )
        .await;
    }
    let store = SqliteStore::open(&path).await?;
    let result = execute(&command, &options, &store).await;
    store.close().await;
    result
}

async fn execute(
    command: &str,
    options: &BTreeMap<String, String>,
    store: &SqliteStore,
) -> Result<Value, Error> {
    let key_path = store.data_directory().join("keys.toml");
    if command == "init" {
        let ring = if key_path.exists() {
            Keyring::load(&key_path)?
        } else {
            if store.registered_count().await? != 0 {
                return Err(
                    "existing accounts require their original keyring; initialization refused"
                        .into(),
                );
            }
            Keyring::create_new(&key_path)?
        };
        verify_keys(store, &ring).await?;
        return Ok(json!({"initialized": true}));
    }
    match command {
        "backup-create" => {
            backup::create(
                store,
                std::path::Path::new(options.get("--output").ok_or("--output is required")?),
            )
            .await
        }
        "invite" => {
            let count: u32 = options
                .get("--count")
                .map(|v| v.parse())
                .transpose()?
                .unwrap_or(1);
            let uses: u32 = options
                .get("--uses")
                .map(|v| v.parse())
                .transpose()?
                .unwrap_or(3);
            if !(1..=1000).contains(&count) || !(1..=1000).contains(&uses) {
                return Err("count and uses must be between 1 and 1000".into());
            }
            let mut codes = Vec::new();
            for _ in 0..count {
                let code = new_invitation()?;
                store
                    .create_invite(
                        invitation_digest(&code).ok_or("invitation generation failed")?,
                        uses,
                    )
                    .await?;
                codes.push(code.to_string());
            }
            Ok(json!({"codes": codes, "uses_per_code": uses}))
        }
        "account-create" => {
            let ring = Keyring::load(&key_path)?;
            verify_keys(store, &ring).await?;
            let mut source = Zeroizing::new(Vec::new());
            std::io::stdin().take(4097).read_to_end(&mut source)?;
            if source.len() > 4096 {
                return Err("registration input is too long".into());
            }
            let input: AccountInput =
                serde_json::from_slice(&source).map_err(|_| "invalid registration JSON")?;
            let registration = Registration {
                requested_uin: input.requested_uin.map(UserId::new).transpose()?,
                invite_digest: invitation_digest(&input.invite)
                    .ok_or("invalid invitation format")?,
                password: LegacyPassword::new(Zeroizing::new(input.password.as_bytes().to_vec()))?,
                email: input.email,
            };
            let user = store.register_with_invite(registration, &ring).await?;
            Ok(json!({"uin": user.get()}))
        }
        "retire" => {
            let user = UserId::new(options.get("--uin").ok_or("--uin is required")?.parse()?)?;
            store.retire_account(user).await?;
            Ok(json!({"retired_uin": user.get(), "number_remains_reserved": true}))
        }
        "stats" => Ok(json!({"registered": store.registered_count().await?})),
        _ => Err("unknown command".into()),
    }
}

async fn verify_keys(store: &SqliteStore, ring: &Keyring) -> Result<(), Error> {
    for id in store.required_key_ids().await? {
        if !ring.has_key(&id) {
            return Err("keyring does not contain every key required by this database".into());
        }
    }
    for probe in store.credential_key_probes().await? {
        ring.open(probe.user, &probe.secret)?;
    }
    Ok(())
}
