// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
mod config;
use rgg_http::HttpState;
use rgg_store_sqlite::SqliteStore;
use std::{future::IntoFuture, path::PathBuf, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
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
    let listener = match tokio::net::TcpListener::bind(config.http_bind).await {
        Ok(listener) => listener,
        Err(error) => {
            store.close().await;
            return Err(error.into());
        }
    };
    let address = listener.local_addr()?;
    let state = HttpState::new(store.clone());
    let stop = CancellationToken::new();
    let server = axum::serve(listener, rgg_http::router(state.clone()))
        .with_graceful_shutdown(stop.clone().cancelled_owned())
        .into_future();
    tokio::pin!(server);
    state.set_ready(true);
    info!(%address, version = env!("CARGO_PKG_VERSION"), "HTTP listening (local development)");
    let outcome = tokio::select! {
        result = &mut server => result.map_err(Error::from),
        signal = shutdown_signal() => {
            state.set_ready(false);
            stop.cancel();
            info!("draining connections");
            match tokio::time::timeout(Duration::from_secs(config.shutdown_seconds), &mut server).await {
                Ok(result) => result.map_err(Error::from),
                Err(_) => { warn!("HTTP drain deadline exceeded"); Ok(()) }
            }.and(signal.map_err(Error::from))
        }
    };
    state.set_ready(false);
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
