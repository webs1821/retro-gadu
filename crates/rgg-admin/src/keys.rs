// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Key publication precedes row re-encryption. Old keys are never removed.
use crate::{Error, backup};
use rgg_credentials::Keyring;
use rgg_store_sqlite::SqliteStore;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
fn pending(counts: &BTreeMap<String, u64>, ring: &Keyring) -> u64 {
    counts
        .iter()
        .filter(|(key, _)| key.as_str() != ring.active_id())
        .map(|(_, n)| n)
        .sum()
}
fn fault(stage: &str) {
    #[cfg(feature = "test-hooks")]
    if std::env::var("RGG_TEST_KEY_CRASH").as_deref() == Ok(stage) {
        std::process::exit(86);
    }
    #[cfg(not(feature = "test-hooks"))]
    let _ = stage;
}
fn publish(root: &Path) -> Result<(), Error> {
    fault("before_key_publish");
    std::fs::rename(root.join("keys.pending"), root.join("keys.toml"))?;
    #[cfg(unix)]
    std::fs::File::open(root)?.sync_all()?;
    fault("after_key_publish");
    Ok(())
}
fn batch_size() -> u32 {
    #[cfg(feature = "test-hooks")]
    if std::env::var("RGG_TEST_KEY_CRASH").as_deref() == Ok("after_rekey_batch") {
        return 1;
    }
    64
}
pub async fn run(
    command: &str,
    store: &SqliteStore,
    backup_path: Option<&Path>,
) -> Result<Value, Error> {
    let root = store.data_directory();
    let path = root.join("keys.toml");
    let mut ring = Keyring::load(&path)?;
    let counts = store.inspect_credentials(&ring).await?;
    let pending_path = root.join("keys.pending");
    let has_pending = std::fs::symlink_metadata(&pending_path)
        .map(|_| true)
        .or_else(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(e)
            }
        })?;
    if command == "keys-status" {
        return Ok(
            json!({"active_key_id":ring.active_id(),"retained_key_ids":ring.key_ids(),"accounts_per_key":counts,"remaining":pending(&counts,&ring),"pending_key_file":has_pending}),
        );
    }
    if command == "keys-rotate" {
        if has_pending || pending(&counts, &ring) != 0 {
            return Err("unfinished rotation; use keys-resume".into());
        }
        let next = ring.successor()?;
        backup::create(store, backup_path.ok_or("--backup is required")?).await?;
        next.write_new(&pending_path)?;
        publish(root)?;
        ring = next;
    } else if command == "keys-resume" {
        if has_pending {
            let next = Keyring::load(&pending_path)?;
            if !next.is_successor_of(&ring) {
                return Err("pending keyring is not a retaining successor".into());
            }
            store.inspect_credentials(&next).await?;
            publish(root)?;
            ring = next;
        }
    } else {
        return Err("unknown key operation".into());
    }
    let mut changed = 0;
    loop {
        let n = store
            .rekey_batch(&ring, ring.active_id(), batch_size())
            .await?;
        if n == 0 {
            break;
        }
        changed += n;
        fault("after_rekey_batch");
    }
    let counts = store.inspect_credentials(&ring).await?;
    Ok(
        json!({"active_key_id":ring.active_id(),"retained_key_ids":ring.key_ids(),"rewrapped":changed,"remaining":pending(&counts,&ring)}),
    )
}
