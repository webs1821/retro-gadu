// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
mod config;
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
    let store = Arc::new(SqliteStore::open(&config.data_dir).await?);
    let prepared = async {
        let listener = tokio::net::TcpListener::bind(config.http_bind).await?;
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
            let hub = Arc::new(SessionHub::new(
                store.clone(),
                Arc::new(SystemClock),
                config.queue.clone(),
            ));
            let context = Arc::new(rgg_transport::server::Context {
                hub,
                authentication: Authenticator::new(store.clone(), ring)?,
                limits: config.network.clone(),
            });
            Some((tokio::net::TcpListener::bind(bind).await?, context))
        } else {
            None
        };
        Ok::<_, Error>((listener, gg))
    }
    .await;
    let (listener, gg) = match prepared {
        Ok(value) => value,
        Err(error) => {
            store.close().await;
            return Err(error);
        }
    };
    let address = listener.local_addr()?;
    let state = HttpState::new(store.clone());
    let stop = CancellationToken::new();
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
    let server = axum::serve(listener, rgg_http::router(state.clone()))
        .with_graceful_shutdown(stop.clone().cancelled_owned())
        .into_future();
    tokio::pin!(server);
    state.set_ready(true);
    info!(%address, version = env!("CARGO_PKG_VERSION"), "HTTP listening (local development)");
    let mut http_done = false;
    let mut gg_done = false;
    let outcome = tokio::select! {
        result = &mut server => {http_done=true;result.map_err(Error::from)},
        result = &mut gg_task => {gg_done=true;result.map_err(Error::from).and_then(|r|r.map_err(Error::from))},
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
