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
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(root.join("rgg.lock"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_lives_until_owner_drops_and_can_then_be_reacquired() {
        let root = tempfile::tempdir().unwrap();
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
        let root = tempfile::tempdir().unwrap();
        let _owner = DataDirectory::acquire(root.path()).unwrap();
        assert!(matches!(
            DataDirectory::acquire(&root.path().join(".")),
            Err(DirectoryError::InUse)
        ));
    }
}
