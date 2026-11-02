// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821

use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

/// The open handle, not a PID file, owns the OS lock. Never unlink a live lock file.
pub struct DataDirectory {
    root: PathBuf,
    _lock: File,
}

#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
    #[error("another process owns this data directory")]
    InUse,
    #[error(
        "data directory must restrict access to its owner (Unix mode 0700); fix permissions before opening"
    )]
    UnsafeDirectory,
    #[error(
        "data directory contains an unsafe lock or database file (link, special file or lock permissions)"
    )]
    UnsafeEntry,
    #[error("data directory I/O failed")]
    Io(#[from] std::io::Error),
}

impl DataDirectory {
    pub fn acquire(path: &Path) -> Result<Self, DirectoryError> {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(path)?;
        let root = path.canonicalize()?;
        // Never repair existing permissions silently: the operator must choose a private location.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(&root)?.permissions().mode() & 0o077 != 0 {
                return Err(DirectoryError::UnsafeDirectory);
            }
        }
        for name in [
            "rgg.sqlite",
            "rgg.sqlite-wal",
            "rgg.sqlite-shm",
            "rgg.sqlite-journal",
        ] {
            regular_entry(&root.join(name), false)?;
        }
        regular_entry(&root.join("rgg.lock"), true)?;
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(root.join("rgg.lock"))?;
        // The directory is private on Unix. Its owner is trusted not to replace entries
        // concurrently; the open handle must still be a regular, private lock file.
        regular_metadata(&file.metadata()?, true)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { root, _lock: file }),
            Err(std::fs::TryLockError::WouldBlock) => Err(DirectoryError::InUse),
            Err(std::fs::TryLockError::Error(error)) => Err(DirectoryError::Io(error)),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn regular_entry(path: &Path, lock: bool) -> Result<(), DirectoryError> {
    match fs::symlink_metadata(path) {
        Ok(meta) => regular_metadata(&meta, lock),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn regular_metadata(meta: &fs::Metadata, lock: bool) -> Result<(), DirectoryError> {
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(DirectoryError::UnsafeEntry);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.nlink() != 1 || (lock && meta.permissions().mode() & 0o077 != 0) {
            return Err(DirectoryError::UnsafeEntry);
        }
    }
    #[cfg(not(unix))]
    let _ = lock; // Windows ACL qualification is separate; file-type guards still apply.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_lives_until_owner_drops_and_can_then_be_reacquired() {
        let root = crate::test_directory();
        let owner = DataDirectory::acquire(root.path()).unwrap();
        assert!(matches!(
            DataDirectory::acquire(root.path()),
            Err(DirectoryError::InUse)
        ));
        drop(owner);
        assert!(DataDirectory::acquire(root.path()).is_ok());
    }

    #[test]
    fn aliases_of_a_directory_do_not_create_two_owners() {
        let root = crate::test_directory();
        let _owner = DataDirectory::acquire(root.path()).unwrap();
        assert!(matches!(
            DataDirectory::acquire(&root.path().join(".")),
            Err(DirectoryError::InUse)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn public_existing_directories_are_refused_without_creating_a_lock() {
        use std::os::unix::fs::PermissionsExt;
        for mode in [0o755, 0o750, 0o770, 0o777] {
            let root = crate::test_directory();
            fs::set_permissions(root.path(), fs::Permissions::from_mode(mode)).unwrap();
            assert!(matches!(
                DataDirectory::acquire(root.path()),
                Err(DirectoryError::UnsafeDirectory)
            ));
            assert!(!root.path().join("rgg.lock").exists());
            assert_eq!(
                fs::metadata(root.path()).unwrap().permissions().mode() & 0o777,
                mode
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn lock_links_permissions_and_database_aliases_are_refused() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = crate::test_directory();
        let outside = root.path().join("outside");
        fs::write(&outside, b"KEEP").unwrap();
        let data = root.path().join("data");
        fs::DirBuilder::new().create(&data).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).unwrap();
        for name in [
            "rgg.lock",
            "rgg.sqlite",
            "rgg.sqlite-wal",
            "rgg.sqlite-shm",
            "rgg.sqlite-journal",
        ] {
            let path = data.join(name);
            symlink(&outside, &path).unwrap();
            assert!(matches!(
                DataDirectory::acquire(&data),
                Err(DirectoryError::UnsafeEntry)
            ));
            fs::remove_file(&path).unwrap();
            fs::hard_link(&outside, &path).unwrap();
            assert!(matches!(
                DataDirectory::acquire(&data),
                Err(DirectoryError::UnsafeEntry)
            ));
            fs::remove_file(&path).unwrap();
        }
        fs::write(data.join("rgg.lock"), b"").unwrap();
        fs::set_permissions(data.join("rgg.lock"), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            DataDirectory::acquire(&data),
            Err(DirectoryError::UnsafeEntry)
        ));
        assert_eq!(fs::read(&outside).unwrap(), b"KEEP");
        fs::set_permissions(data.join("rgg.lock"), fs::Permissions::from_mode(0o600)).unwrap();
        assert!(DataDirectory::acquire(&data).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlink_keeps_the_same_private_owner_lock() {
        use std::os::unix::fs::symlink;
        let root = crate::test_directory();
        let target = root.path().join("data");
        let owner = DataDirectory::acquire(&target).unwrap();
        let alias = root.path().join("alias");
        symlink(&target, &alias).unwrap();
        assert!(matches!(
            DataDirectory::acquire(&alias),
            Err(DirectoryError::InUse)
        ));
        drop(owner);
        assert!(DataDirectory::acquire(&alias).is_ok());
    }
}
