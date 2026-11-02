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
    pub gg_advertise: Option<rgg_http::AdvertisedEndpoint>,
    pub metrics_bind: Option<SocketAddr>,
    #[serde(default)]
    pub legacy_discovery: bool,
    #[serde(default)]
    pub web_registration: bool,
    #[serde(default)]
    pub web_password_change: bool,
    #[serde(default)]
    pub web_public_stats: bool,
    pub web_proxy: Option<rgg_http::WebProxy>,
    pub downloads_manifest: Option<PathBuf>,
    pub client_patcher: Option<rgg_http::patcher::Settings>,
    #[serde(default)]
    pub http: crate::http_listener::Limits,
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
    #[error("invalid network, HTTP or queue limits")]
    Limits,
    #[error("legacy_discovery requires a local IPv4 GG listener")]
    LegacyDiscovery,
    #[error("gg_advertise requires legacy_discovery and a local IPv4 GG listener")]
    AdvertisedEndpoint,
    #[error("web_registration requires a GG listener and initialized account keyring")]
    WebRegistration,
    #[error("web_password_change requires a GG listener and initialized account keyring")]
    WebPasswordChange,
    #[error("web_proxy requires an enabled website feature")]
    WebProxy,
    #[error("metrics must use a separate loopback endpoint")]
    MetricsEndpoint,
    #[error(
        "client_patcher.default_host must be 1..16 ASCII alphanumeric characters with single dot/hyphen separators"
    )]
    ClientPatcher,
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
            || config.metrics_bind.is_some_and(|a| !a.ip().is_loopback())
        {
            return Err(ConfigError::PublicBind);
        }
        if !(1..=30).contains(&config.shutdown_seconds) {
            return Err(ConfigError::Shutdown);
        }
        if config.legacy_discovery && !config.gg_bind.is_some_and(|a| a.is_ipv4()) {
            return Err(ConfigError::LegacyDiscovery);
        }
        if config.gg_advertise.is_some() && !config.legacy_discovery {
            return Err(ConfigError::AdvertisedEndpoint);
        }
        if config.web_registration && config.gg_bind.is_none() {
            return Err(ConfigError::WebRegistration);
        }
        if config.web_password_change && config.gg_bind.is_none() {
            return Err(ConfigError::WebPasswordChange);
        }
        if config.web_proxy.is_some()
            && !(config.web_registration
                || config.web_password_change
                || config.web_public_stats
                || config.downloads_manifest.is_some()
                || config.client_patcher.is_some())
        {
            return Err(ConfigError::WebProxy);
        }
        if config
            .metrics_bind
            .is_some_and(|a| a.port() != 0 && (a == config.http_bind || Some(a) == config.gg_bind))
        {
            return Err(ConfigError::MetricsEndpoint);
        }
        let q = &config.queue;
        if config
            .client_patcher
            .as_ref()
            .is_some_and(|settings| !settings.valid())
        {
            return Err(ConfigError::ClientPatcher);
        }
        if !config.network.valid()
            || !config.http.valid()
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
        if let Some(path) = &mut config.downloads_manifest {
            if path.as_os_str().is_empty() {
                return Err(ConfigError::Syntax);
            }
            if path.is_relative() {
                *path = base.join(&*path);
            }
        }
        Ok(config)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn https_proxy_is_opt_in_validated_and_requires_website_features() {
        let base = "data_dir='data'\nhttp_bind='127.0.0.1:0'\n";
        let profile = "[web_proxy]\norigin='https://rgg.test'\ntrusted_proxy='127.0.0.2'\n";
        assert!(
            Config::parse(base, Path::new("."))
                .unwrap()
                .web_proxy
                .is_none()
        );
        assert!(matches!(
            Config::parse(&format!("{base}{profile}"), Path::new(".")),
            Err(ConfigError::WebProxy)
        ));
        assert!(
            Config::parse(
                &format!("{base}web_public_stats=true\n{profile}"),
                Path::new(".")
            )
            .is_ok()
        );
        for profile in [
            "origin='https://rgg.test'",
            "trusted_proxy='127.0.0.2'",
            "origin='http://rgg.test'\ntrusted_proxy='127.0.0.2'",
            "origin='https://rgg.test'\ntrusted_proxy='10.0.0.1'",
            "origin='https://rgg.test'\ntrusted_proxy='127.0.0.2'\ntrust_all=true",
        ] {
            assert!(
                Config::parse(
                    &format!("{base}web_public_stats=true\n[web_proxy]\n{profile}"),
                    Path::new(".")
                )
                .is_err()
            );
        }
    }
    #[test]
    fn password_website_requires_gg_and_is_disabled_by_default() {
        let base = "data_dir='data'\nhttp_bind='127.0.0.1:0'\n";
        assert!(
            !Config::parse(base, Path::new("."))
                .unwrap()
                .web_password_change
        );
        assert!(matches!(
            Config::parse(&format!("{base}web_password_change=true"), Path::new(".")),
            Err(ConfigError::WebPasswordChange)
        ));
        assert!(
            Config::parse(
                &format!("{base}web_password_change=true\ngg_bind='127.0.0.1:0'"),
                Path::new(".")
            )
            .is_ok()
        );
    }
    #[test]
    fn browser_patcher_is_opt_in_and_validates_hostname_before_database_open() {
        let base = "data_dir='data'\nhttp_bind='127.0.0.1:0'\n";
        assert!(
            Config::parse(base, Path::new("."))
                .unwrap()
                .client_patcher
                .is_none()
        );
        for host in ["127.0.0.1", "rgg.local", "Ab-c.def", "abcdefghijklmnop"] {
            assert!(
                Config::parse(
                    &format!("{base}[client_patcher]\ndefault_host='{host}'"),
                    Path::new(".")
                )
                .is_ok()
            );
        }
        for host in [
            "",
            "abcdefghijklmnopq",
            "https://rgg",
            "a..b",
            "a--b",
            "a.-b",
            "-a",
            "a.",
            "żółć",
            "a:80",
            "a/b",
            "a\"b",
        ] {
            assert!(
                Config::parse(
                    &format!("{base}[client_patcher]\ndefault_host='{host}'"),
                    Path::new(".")
                )
                .is_err()
            );
        }
        for settings in ["", "default_host='rgg.local'\nunknown=true"] {
            assert!(
                Config::parse(
                    &format!("{base}[client_patcher]\n{settings}"),
                    Path::new(".")
                )
                .is_err()
            );
        }
    }
    #[test]
    fn metrics_are_opt_in_loopback_and_separate_from_client_services() {
        let base = "data_dir='data'\nhttp_bind='127.0.0.1:8088'\ngg_bind='127.0.0.1:8074'\n";
        assert!(
            Config::parse(base, Path::new("."))
                .unwrap()
                .metrics_bind
                .is_none()
        );
        for address in [
            "0.0.0.0:9090",
            "[::]:9090",
            "192.168.1.2:9090",
            "127.0.0.1:8088",
            "127.0.0.1:8074",
        ] {
            assert!(
                Config::parse(&format!("{base}metrics_bind='{address}'"), Path::new(".")).is_err()
            );
        }
        for address in ["127.0.0.1:9090", "[::1]:9090", "127.0.0.1:0"] {
            assert!(
                Config::parse(&format!("{base}metrics_bind='{address}'"), Path::new(".")).is_ok()
            );
        }
    }
    #[test]
    fn http_limits_are_bounded_and_reject_unknown_fields() {
        let base = "data_dir='data'\nhttp_bind='127.0.0.1:0'\n[http]\n";
        assert!(Config::parse(base, Path::new(".")).is_ok());
        for extra in [
            "max_connections=0",
            "per_ip=65",
            "idle_seconds=0",
            "write_seconds=121",
            "lifetime_seconds=14",
            "unknown=1",
        ] {
            assert!(Config::parse(&format!("{base}{extra}"), Path::new(".")).is_err());
        }
    }
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
    fn advertised_destination_is_explicit_and_never_relaxes_listener_policy() {
        let base = "data_dir='data'\nhttp_bind='127.0.0.1:0'\ngg_bind='127.0.0.1:0'\n";
        assert!(
            Config::parse(base, Path::new("."))
                .unwrap()
                .gg_advertise
                .is_none()
        );
        assert!(matches!(
            Config::parse(
                &format!("{base}gg_advertise='198.51.100.7:8074'"),
                Path::new(".")
            ),
            Err(ConfigError::AdvertisedEndpoint)
        ));
        for value in ["127.0.0.2:18074", "198.51.100.7:8074"] {
            assert!(
                Config::parse(
                    &format!("{base}legacy_discovery=true\ngg_advertise='{value}'"),
                    Path::new(".")
                )
                .is_ok()
            );
        }
        for value in [
            "[::1]:8074",
            "example.invalid:8074",
            "127.0.0.1:0",
            "0.0.0.0:8074",
            "255.255.255.255:8074",
            "224.0.0.1:8074",
        ] {
            assert!(
                Config::parse(
                    &format!("{base}legacy_discovery=true\ngg_advertise='{value}'"),
                    Path::new(".")
                )
                .is_err()
            );
        }
        let public = base.replace("gg_bind='127.0.0.1:0'", "gg_bind='0.0.0.0:8074'");
        assert!(matches!(
            Config::parse(
                &format!("{public}legacy_discovery=true\ngg_advertise='198.51.100.7:8074'"),
                Path::new(".")
            ),
            Err(ConfigError::PublicBind)
        ));
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
