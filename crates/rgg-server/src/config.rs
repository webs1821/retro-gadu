// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use serde::Deserialize;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub data_dir: PathBuf,
    pub http_bind: SocketAddr,
    pub gg_bind: Option<SocketAddr>,
    #[serde(default)]
    pub legacy_discovery: bool,
    #[serde(default)]
    pub web_registration: bool,
    #[serde(default)]
    pub web_public_stats: bool,
    #[serde(default)]
    pub network: rgg_transport::server::Limits,
    #[serde(default)]
    pub queue: rgg_application::messaging::QueueLimits,
    #[serde(default = "shutdown_default")]
    pub shutdown_seconds: u64,
}
fn shutdown_default() -> u64 {
    5
}
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read configuration")]
    Io(#[from] std::io::Error),
    // TOML errors can quote input: never forward their text to logs.
    #[error("invalid configuration syntax or unknown field")]
    Syntax,
    #[error("development server must bind to a loopback IP address")]
    PublicBind,
    #[error("shutdown_seconds must be between 1 and 30")]
    Shutdown,
    #[error("data_dir must not be empty")]
    DataDirectory,
    #[error("invalid network or queue limits")]
    Limits,
    #[error("legacy_discovery requires a local IPv4 GG listener")]
    LegacyDiscovery,
    #[error("web_registration requires a GG listener and initialized account keyring")]
    WebRegistration,
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let source = std::fs::read_to_string(path)?;
        Self::parse(&source, path.parent().unwrap_or(Path::new(".")))
    }
    fn parse(source: &str, base: &Path) -> Result<Self, ConfigError> {
        let mut config: Self = toml::from_str(source).map_err(|_| ConfigError::Syntax)?;
        if !config.http_bind.ip().is_loopback()
            || config.gg_bind.is_some_and(|a| !a.ip().is_loopback())
        {
            return Err(ConfigError::PublicBind);
        }
        if !(1..=30).contains(&config.shutdown_seconds) {
            return Err(ConfigError::Shutdown);
        }
        if config.legacy_discovery && !config.gg_bind.is_some_and(|a| a.is_ipv4()) {
            return Err(ConfigError::LegacyDiscovery);
        }
        if config.web_registration && config.gg_bind.is_none() {
            return Err(ConfigError::WebRegistration);
        }
        let q = &config.queue;
        if !config.network.valid()
            || !(1..=10000).contains(&q.per_recipient_messages)
            || !(1..=10000).contains(&q.per_sender_messages)
            || !(1..=100000).contains(&q.global_messages)
            || !(1024..=64 * 1024 * 1024).contains(&q.per_recipient_bytes)
            || !(1024..=512 * 1024 * 1024).contains(&q.global_bytes)
            || !(1..=30 * 86400).contains(&q.ttl_seconds)
        {
            return Err(ConfigError::Limits);
        }
        if config.data_dir.as_os_str().is_empty() {
            return Err(ConfigError::DataDirectory);
        }
        if config.data_dir.is_relative() {
            config.data_dir = base.join(&config.data_dir);
        }
        Ok(config)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn website_requires_the_initialized_gg_account_runtime() {
        assert!(matches!(
            Config::parse(
                "data_dir='data'\nhttp_bind='127.0.0.1:0'\nweb_registration=true",
                Path::new(".")
            ),
            Err(ConfigError::WebRegistration)
        ));
        assert!(Config::parse("data_dir='data'\nhttp_bind='127.0.0.1:0'\ngg_bind='127.0.0.1:0'\nweb_registration=true",Path::new(".")).is_ok());
    }
    #[test]
    fn discovery_requires_an_ipv4_gg_listener() {
        for extra in ["", "\ngg_bind='[::1]:0'"] {
            assert!(matches!(
                Config::parse(
                    &format!(
                        "data_dir='data'\nhttp_bind='127.0.0.1:0'\nlegacy_discovery=true{extra}"
                    ),
                    Path::new(".")
                ),
                Err(ConfigError::LegacyDiscovery)
            ));
        }
        assert!(Config::parse("data_dir='data'\nhttp_bind='127.0.0.1:0'\ngg_bind='127.0.0.1:0'\nlegacy_discovery=true",Path::new(".")).is_ok());
    }
    #[test]
    fn paths_are_relative_to_configuration_and_unknown_options_are_errors() {
        let config = Config::parse(
            "data_dir='data'\nhttp_bind='127.0.0.1:0'",
            Path::new("/config"),
        )
        .unwrap();
        assert_eq!(config.data_dir, Path::new("/config/data"));
        assert!(matches!(
            Config::parse(
                "data_dir='data'\nhttp_bind='127.0.0.1:0'\nhtp_bind='oops'",
                Path::new(".")
            ),
            Err(ConfigError::Syntax)
        ));
    }
    #[test]
    fn local_development_cannot_accidentally_listen_on_the_lan_or_internet() {
        for bind in ["0.0.0.0:8080", "[::]:8080", "192.168.1.2:8080"] {
            let text = format!("data_dir='data'\nhttp_bind='{bind}'");
            assert!(matches!(
                Config::parse(&text, Path::new(".")),
                Err(ConfigError::PublicBind)
            ));
        }
        assert!(Config::parse("data_dir='data'\nhttp_bind='[::1]:8080'", Path::new(".")).is_ok());
        assert!(matches!(
            Config::parse("data_dir=''\nhttp_bind='127.0.0.1:8080'", Path::new(".")),
            Err(ConfigError::DataDirectory)
        ));
        assert!(matches!(
            Config::parse(
                "data_dir='data'\nhttp_bind='127.0.0.1:8080'\nshutdown_seconds=0",
                Path::new(".")
            ),
            Err(ConfigError::Shutdown)
        ));
    }
}
