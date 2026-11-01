// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Offline complete backup sets. A manifest commits a backup; a marker fences partial restores.
use crate::Error;
use rgg_credentials::Keyring;
use rgg_store_sqlite::{
    SqliteStore,
    backup::{MAX_SNAPSHOT_BYTES, SnapshotInfo, inspect_snapshot},
    directory::DataDirectory,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use zeroize::Zeroizing;
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Fingerprint {
    size: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    server_version: String,
    schema: i64,
    accounts: u64,
    key_ids: Vec<String>,
    database: Fingerprint,
    keys: Fingerprint,
}
fn private_dir_new(path: &Path) -> Result<(), Error> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|_| "destination must not exist and its parent must exist")?;
    Ok(())
}
fn private_new(path: &Path) -> Result<File, Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options
        .open(path)
        .map_err(|_| "cannot create private backup file")?)
}
fn regular(path: &Path, max: u64) -> Result<File, Error> {
    let meta = fs::symlink_metadata(path).map_err(|_| "backup file missing")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > max {
        return Err("backup file is not a bounded regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("backup file permissions must be owner-only".into());
        }
    }
    Ok(File::open(path).map_err(|_| "backup file unavailable")?)
}
fn digest_copy(source: &Path, target: Option<&Path>, max: u64) -> Result<Fingerprint, Error> {
    let mut input = regular(source, max)?.take(max + 1);
    let mut output = target.map(private_new).transpose()?;
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = Zeroizing::new([0u8; 65536]);
    loop {
        let n = input.read(&mut *buffer).map_err(|_| "backup read failed")?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > max {
            return Err("backup size limit exceeded".into());
        }
        hash.update(&buffer[..n]);
        if let Some(out) = &mut output {
            out.write_all(&buffer[..n])
                .map_err(|_| "backup write failed")?;
        }
    }
    if let Some(out) = output {
        out.sync_all().map_err(|_| "backup sync failed")?;
    }
    Ok(Fingerprint {
        size,
        sha256: hex::encode(hash.finalize()),
    })
}
fn sync_dir(path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn report(info: &SnapshotInfo) -> Value {
    json!({"verified":true,"schema":info.schema,"accounts":info.accounts,"required_key_ids":info.key_ids})
}
async fn verify_files(root: &Path, manifest: &Manifest) -> Result<SnapshotInfo, Error> {
    if manifest.format != 1
        || manifest.schema != rgg_store_sqlite::SCHEMA_VERSION
        || manifest.server_version.len() > 80
        || manifest.key_ids.len() > 32
    {
        return Err("unsupported backup manifest".into());
    }
    if digest_copy(&root.join("rgg.sqlite"), None, MAX_SNAPSHOT_BYTES)? != manifest.database
        || digest_copy(&root.join("keys.toml"), None, 16384)? != manifest.keys
    {
        return Err("backup checksum mismatch".into());
    }
    let ring = Keyring::load(&root.join("keys.toml"))?;
    let info = inspect_snapshot(&root.join("rgg.sqlite"), &ring).await?;
    if info.schema != manifest.schema
        || info.accounts != manifest.accounts
        || info.key_ids != manifest.key_ids
    {
        return Err("backup metadata mismatch".into());
    }
    Ok(info)
}
async fn read_verified(root: &Path) -> Result<(Manifest, SnapshotInfo), Error> {
    let meta = fs::symlink_metadata(root).map_err(|_| "backup directory missing")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("backup must be a regular directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("backup directory permissions must be owner-only".into());
        }
    }
    for entry in fs::read_dir(root)? {
        let name = entry?.file_name();
        if !["manifest.json", "rgg.sqlite", "keys.toml"]
            .iter()
            .any(|n| name == *n)
        {
            return Err("incomplete backup or unexpected backup file".into());
        }
    }
    let mut raw = Vec::new();
    regular(&root.join("manifest.json"), 16384)?
        .take(16385)
        .read_to_end(&mut raw)?;
    if raw.len() > 16384 {
        return Err("backup manifest too large".into());
    }
    let manifest: Manifest = serde_json::from_slice(&raw).map_err(|_| "invalid backup manifest")?;
    let info = verify_files(root, &manifest).await?;
    Ok((manifest, info))
}
pub async fn verify(root: &Path) -> Result<Value, Error> {
    let (_, info) = read_verified(root).await?;
    Ok(report(&info))
}
pub async fn create(store: &SqliteStore, output: &Path) -> Result<Value, Error> {
    // An initialized keyring is mandatory even for an empty database.
    let source_keys = store.data_directory().join("keys.toml");
    Keyring::load(&source_keys)?;
    private_dir_new(output)?;
    let output = output.canonicalize()?;
    let keys = digest_copy(&source_keys, Some(&output.join("keys.toml")), 16384)?;
    let database_path = output.join("rgg.sqlite");
    private_new(&database_path)?.sync_all()?;
    store.snapshot_into(&database_path).await?;
    File::open(&database_path)?.sync_all()?;
    let ring = Keyring::load(&output.join("keys.toml"))?;
    let info = inspect_snapshot(&database_path, &ring).await?;
    let database = digest_copy(&database_path, None, MAX_SNAPSHOT_BYTES)?;
    let manifest = Manifest {
        format: 1,
        server_version: env!("CARGO_PKG_VERSION").into(),
        schema: info.schema,
        accounts: info.accounts,
        key_ids: info.key_ids.clone(),
        database,
        keys,
    };
    let mut file = private_new(&output.join("manifest.pending"))?;
    file.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    file.sync_all()?;
    drop(file);
    sync_dir(&output)?;
    fault("before_backup_commit");
    fs::rename(
        output.join("manifest.pending"),
        output.join("manifest.json"),
    )?;
    sync_dir(&output)?;
    if let Some(parent) = output.parent() {
        sync_dir(parent)?;
    }
    Ok(json!({"created":true,"verification":report(&info)}))
}
pub async fn restore(backup: &Path, target: &Path) -> Result<Value, Error> {
    let (manifest, _) = read_verified(backup).await?;
    private_dir_new(target)?;
    let directory = DataDirectory::acquire(target)?;
    let target = directory.root();
    private_new(&target.join("restore-incomplete"))?.sync_all()?;
    sync_dir(target)?;
    let db = digest_copy(
        &backup.join("rgg.sqlite"),
        Some(&target.join("rgg.sqlite")),
        MAX_SNAPSHOT_BYTES,
    )?;
    let keys = digest_copy(
        &backup.join("keys.toml"),
        Some(&target.join("keys.toml")),
        16384,
    )?;
    if db != manifest.database || keys != manifest.keys {
        return Err("backup changed while copying; restore incomplete".into());
    }
    let info = verify_files(target, &manifest).await?;
    sync_dir(target)?;
    fault("before_restore_commit");
    fs::remove_file(target.join("restore-incomplete"))?;
    sync_dir(target)?;
    if let Some(parent) = target.parent() {
        sync_dir(parent)?;
    }
    drop(directory);
    Ok(json!({"restored":true,"verification":report(&info)}))
}
fn fault(stage: &str) {
    #[cfg(feature = "test-hooks")]
    if std::env::var("RGG_TEST_BACKUP_CRASH").as_deref() == Ok(stage) {
        std::process::exit(86);
    }
    #[cfg(not(feature = "test-hooks"))]
    let _ = stage;
}
