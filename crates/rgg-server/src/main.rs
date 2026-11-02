// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
mod config;
mod http_listener;
mod process_metrics;
use axum::serve::ListenerExt;
use rgg_application::{
    accounts::{AccountRepository, SecretProtector},
    messaging::{Authenticator, Clock},
    sessions::SessionHub,
};
use rgg_http::HttpState;
use rgg_store_sqlite::SqliteStore;
use std::{future::IntoFuture, path::PathBuf, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
struct SystemClock;
impl Clock for SystemClock {
    fn unix_seconds(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .min(i64::MAX as u64) as i64
    }
}
type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rGG: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rgg_server=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let mut args = std::env::args_os().skip(1);
    let config_file = match args.next().as_deref().and_then(|arg| arg.to_str()) {
        Some("--config") => PathBuf::from(args.next().ok_or("--config requires a path")?),
        Some("--version") if args.next().is_none() => {
            println!("Retro Gadu (rGG) {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help") | None => {
            println!(
                "Retro Gadu — local development\nUsage: rgg-server --config <file.toml>\n       rgg-server --version"
            );
            return Ok(());
        }
        _ => return Err("invalid arguments; use --help".into()),
    };
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    let config = config::Config::load(&config_file)?;
    if config.web_password_change
        && config
            .data_dir
            .join("keys.pending")
            .symlink_metadata()
            .is_ok()
    {
        return Err("resume pending key rotation before enabling password changes".into());
    }
    let downloads = config
        .downloads_manifest
        .as_deref()
        .map(rgg_http::downloads::Catalog::load)
        .transpose()?
        .map(Arc::new);
    let store = Arc::new(SqliteStore::open(&config.data_dir).await?);
    let telemetry = Arc::new(rgg_application::telemetry::RuntimeTelemetry::default());
    let prepared = async {
        let mut registration = None;
        let mut passwords = None;
        let gg = if let Some(bind) = config.gg_bind {
            let ring = Arc::new(rgg_credentials::Keyring::load(
                &config.data_dir.join("keys.toml"),
            )?);
            for id in store.required_key_ids().await? {
                if !ring.has_key(&id) {
                    return Err::<_, Error>("required credential key missing".into());
                }
            }
            for probe in store.credential_key_probes().await? {
                ring.open(probe.user, &probe.secret)?;
            }
            if config.web_registration {
                registration = Some(Arc::new(
                    rgg_application::registration::RegistrationService::new(
                        store.clone(),
                        ring.clone(),
                        Arc::new(rgg_credentials::InvitationCodes),
                    ),
                ));
            }
            let hub = Arc::new(SessionHub::new(
                store.clone(),
                Arc::new(SystemClock),
                config.queue.clone(),
            ));
            if config.web_password_change {
                passwords = Some(Arc::new(rgg_application::password::PasswordService::new(
                    store.clone(),
                    ring.clone(),
                    hub.clone(),
                )?));
            }
            let context = Arc::new(rgg_transport::server::Context {
                telemetry: telemetry.clone(),
                hub,
                authentication: Authenticator::new(store.clone(), ring)?,
                limits: config.network.clone(),
                images: Arc::new(rgg_transfers::Images::default()),
            });
            Some((tokio::net::TcpListener::bind(bind).await?, context))
        } else {
            None
        };
        let listener = tokio::net::TcpListener::bind(config.http_bind).await?;
        let metrics = match config.metrics_bind {
            Some(address) => Some(tokio::net::TcpListener::bind(address).await?),
            None => None,
        };
        Ok::<_, Error>((listener, gg, registration, metrics, passwords))
    }
    .await;
    let (listener, gg, registration, metrics, passwords) = match prepared {
        Ok(value) => value,
        Err(error) => {
            store.close().await;
            return Err(error);
        }
    };
    let address = listener.local_addr()?;
    let state = HttpState::new(store.clone());
    let state = if config.web_registration
        || config.web_public_stats
        || downloads.is_some()
        || config.client_patcher.is_some()
        || passwords.is_some()
    {
        let statistics: Option<Arc<dyn rgg_application::statistics::StatisticsRepository>> =
            if config.web_public_stats {
                Some(store.clone())
            } else {
                None
            };
        state.with_website(
            registration,
            statistics,
            downloads,
            passwords,
            config.client_patcher,
            address,
        )?
    } else {
        state
    };
    let state = if config.legacy_discovery {
        state.with_legacy_discovery(
            gg.as_ref().ok_or("GG listener missing")?.0.local_addr()?,
            config.gg_advertise,
        )?
    } else {
        state
    };
    let stop = CancellationToken::new();
    let metrics = metrics
        .map(|listener| -> Result<_, Error> {
            let address = listener.local_addr()?;
            let router =
                rgg_http::metrics::router(store.clone(), telemetry.clone(), &state, address)?;
            Ok((listener, router, address))
        })
        .transpose()?;
    let process_task = metrics.as_ref().map(|_| {
        info!("Private process measurements enabled");
        tokio::spawn(process_metrics::run(telemetry.clone(), stop.clone()))
    });
    let metrics_stop = stop.clone();
    let mut metrics_task = tokio::spawn(async move {
        if let Some((listener, router, address)) = metrics {
            info!(metrics_endpoint=%address,"Private metrics listening (loopback)");
            let limits = http_listener::Limits {
                max_connections: 8,
                per_ip: 4,
                idle_seconds: 5,
                write_seconds: 5,
                lifetime_seconds: 60,
            };
            axum::serve(
                http_listener::BoundedListener::new(listener, limits)
                    .tap_io(|stream| stream.tcp_nodelay()),
                router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(metrics_stop.cancelled_owned())
            .await
        } else {
            metrics_stop.cancelled().await;
            Ok::<_, std::io::Error>(())
        }
    });
    let hub = gg.as_ref().map(|(_, ctx)| ctx.hub.clone());
    let no_gg_stop = stop.clone();
    let mut gg_task = tokio::spawn(async move {
        if let Some((listener, ctx)) = gg {
            info!(gg_endpoint=%listener.local_addr()?,"GG6 listening (local development)");
            rgg_transport::server::serve(listener, ctx).await
        } else {
            no_gg_stop.cancelled().await;
            Ok(())
        }
    });
    let server = axum::serve(
        http_listener::BoundedListener::new(listener, config.http.clone())
            .tap_io(|stream| stream.tcp_nodelay()),
        rgg_http::router(state.clone())
            .into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(stop.clone().cancelled_owned())
    .into_future();
    tokio::pin!(server);
    state.set_ready(true);
    info!(%address, version = env!("CARGO_PKG_VERSION"), "HTTP listening (local development)");
    let mut http_done = false;
    let mut gg_done = false;
    let mut metrics_done = false;
    let outcome = tokio::select! {
        result = &mut server => {http_done=true;result.map_err(Error::from)},
        result = &mut gg_task => {gg_done=true;result.map_err(Error::from).and_then(|r|r.map_err(Error::from))},
        result = &mut metrics_task => {metrics_done=true;result.map_err(Error::from).and_then(|r|r.map_err(Error::from))},
        signal = shutdown_signal() => signal.map_err(Error::from),
    };
    state.set_ready(false);
    stop.cancel();
    if let Some(hub) = &hub {
        hub.shutdown.cancel();
    }
    info!("draining connections");
    if !http_done
        && tokio::time::timeout(Duration::from_secs(config.shutdown_seconds), &mut server)
            .await
            .is_err()
    {
        warn!("HTTP drain deadline exceeded");
    }
    if !gg_done
        && tokio::time::timeout(Duration::from_secs(16), &mut gg_task)
            .await
            .is_err()
    {
        gg_task.abort();
        let _ = gg_task.await;
    }
    if !metrics_done
        && tokio::time::timeout(
            Duration::from_secs(config.shutdown_seconds),
            &mut metrics_task,
        )
        .await
        .is_err()
    {
        metrics_task.abort();
        let _ = metrics_task.await;
        warn!("Metrics drain deadline exceeded");
    }
    if let Some(mut task) = process_task {
        match tokio::time::timeout(Duration::from_secs(2), &mut task).await {
            Ok(Ok(())) => info!("Process measurement sampler stopped"),
            Ok(Err(_)) => warn!("Process measurement sampler failed"),
            Err(_) => {
                task.abort();
                let _ = task.await;
                warn!("Process measurement sampler drain deadline exceeded");
            }
        }
    }
    if let Some(hub) = hub {
        hub.finish_shutdown().await;
    }
    store.close().await;
    info!("database closed; shutdown complete");
    outcome
}
async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}
