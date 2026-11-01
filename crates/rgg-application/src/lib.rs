// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821

//! Use cases and infrastructure ports. No wire layouts, SQL types or HTTP framework.

pub mod accounts;
pub mod conferences;
pub mod contacts;
pub mod directory;
pub mod images;
pub mod messaging;
pub mod operations;
pub mod password;
pub mod presence;
pub mod registration;
pub mod sessions;
pub mod statistics;

pub mod contracts {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Command {
        Ping,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Event {
        Pong,
    }
}

pub mod ports {
    use async_trait::async_trait;

    #[derive(Debug, thiserror::Error)]
    #[error("storage unavailable")]
    pub struct StorageUnavailable;

    #[async_trait]
    pub trait HealthRepository: Send + Sync {
        async fn check(&self) -> Result<(), StorageUnavailable>;
    }
}

/// The session layer decides whether a control command is permitted in its current state.
pub fn handle_control(command: contracts::Command) -> contracts::Event {
    match command {
        contracts::Command::Ping => contracts::Event::Pong,
    }
}
