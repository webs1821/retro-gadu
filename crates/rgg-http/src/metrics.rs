// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! A separate, opt-in loopback operator surface. No public-router merge.
use crate::HttpState;
use axum::{
    Router,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use rgg_application::operations::{OperationalRepository, OperationalSnapshot};
use std::{
    fmt::Write,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::Instant;

struct Metrics {
    repository: Arc<dyn OperationalRepository>,
    ready: Arc<AtomicBool>,
    authority: String,
    origin: String,
    started: Instant,
    cache: Mutex<Option<(Instant, OperationalSnapshot)>>,
    requests: Semaphore,
}
pub fn router(
    repository: Arc<dyn OperationalRepository>,
    health: &HttpState,
    address: SocketAddr,
) -> Result<Router, &'static str> {
    if !address.ip().is_loopback() || address.port() == 0 {
        return Err("metrics requires a bound loopback listener");
    }
    let state = Arc::new(Metrics {
        repository,
        ready: health.ready.clone(),
        authority: address.to_string(),
        origin: format!("http://{address}"),
        started: Instant::now(),
        cache: Mutex::new(None),
        requests: Semaphore::new(2),
    });
    Ok(Router::new()
        .route("/metrics", get(scrape))
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state))
}
fn exact(headers: &HeaderMap, name: HeaderName, expected: &str, optional: bool) -> bool {
    let mut values = headers.get_all(name).iter();
    match values.next() {
        None => optional,
        Some(value) => value == expected && values.next().is_none(),
    }
}
fn secured(mut response: Response) -> Response {
    for (name, value) in [
        (header::CACHE_CONTROL, "no-store"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; frame-ancestors 'none'",
        ),
    ] {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    response
}
async fn guard(
    State(state): State<Arc<Metrics>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let allowed = remote.ip().is_loopback()
        && exact(headers, header::HOST, &state.authority, false)
        && exact(headers, header::ORIGIN, &state.origin, true)
        && (exact(
            headers,
            HeaderName::from_static("sec-fetch-site"),
            "same-origin",
            true,
        ) || exact(
            headers,
            HeaderName::from_static("sec-fetch-site"),
            "none",
            false,
        ));
    let response = if !allowed {
        (StatusCode::FORBIDDEN, "metrics forbidden\n").into_response()
    } else if request.uri().query().is_some()
        || headers.contains_key(header::TRANSFER_ENCODING)
        || !exact(headers, header::CONTENT_LENGTH, "0", true)
    {
        (
            StatusCode::BAD_REQUEST,
            "metrics expects an empty request\n",
        )
            .into_response()
    } else {
        next.run(request).await
    };
    secured(response)
}
async fn scrape(State(state): State<Arc<Metrics>>) -> Response {
    if !state.ready.load(Ordering::Acquire) {
        return unavailable();
    }
    let Ok(_permit) = state.requests.try_acquire() else {
        return (StatusCode::TOO_MANY_REQUESTS, "metrics busy\n").into_response();
    };
    let sample = tokio::time::timeout(Duration::from_secs(1), async {
        let mut cache = state.cache.lock().await;
        if let Some((at, sample)) = *cache
            && at.elapsed() < Duration::from_millis(500)
        {
            return Ok(sample);
        }
        let at = Instant::now();
        let sample = state.repository.operational_snapshot().await?;
        *cache = Some((at, sample));
        Ok::<_, rgg_application::ports::StorageUnavailable>(sample)
    })
    .await;
    match sample {
        Ok(Ok(sample)) if state.ready.load(Ordering::Acquire) => (
            [(
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            render(sample, state.started.elapsed().as_secs_f64()),
        )
            .into_response(),
        _ => unavailable(),
    }
}
fn unavailable() -> Response {
    (StatusCode::SERVICE_UNAVAILABLE, "metrics unavailable\n").into_response()
}
fn gauge(output: &mut String, name: &str, help: &str, value: impl std::fmt::Display) {
    writeln!(
        output,
        "# HELP {name} {help}\n# TYPE {name} gauge\n{name} {value}"
    )
    .expect("String output");
}
fn render(s: OperationalSnapshot, uptime: f64) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("# HELP rgg_build_info Running binary version.\n# TYPE rgg_build_info gauge\n");
    writeln!(
        out,
        "rgg_build_info{{version=\"{}\"}} 1",
        env!("CARGO_PKG_VERSION")
    )
    .expect("String output");
    gauge(
        &mut out,
        "rgg_uptime_seconds",
        "Time since this metrics listener was prepared.",
        format_args!("{uptime:.3}"),
    );
    gauge(
        &mut out,
        "rgg_database_schema_version",
        "Database schema version.",
        s.schema_version,
    );
    gauge(
        &mut out,
        "rgg_snapshot_timestamp_seconds",
        "Unix time of the database snapshot.",
        s.sampled_at,
    );
    out.push_str(
        "# HELP rgg_accounts Current accounts by activation state.\n# TYPE rgg_accounts gauge\n",
    );
    writeln!(
        out,
        "rgg_accounts{{state=\"active\"}} {}\nrgg_accounts{{state=\"disabled\"}} {}",
        s.active_accounts, s.disabled_accounts
    )
    .expect("String output");
    gauge(
        &mut out,
        "rgg_retired_numbers",
        "Retained number tombstones.",
        s.retired_numbers,
    );
    out.push_str("# HELP rgg_sessions Current valid sessions including invisible users.\n# TYPE rgg_sessions gauge\n");
    writeln!(
        out,
        "rgg_sessions{{phase=\"ready\"}} {}\nrgg_sessions{{phase=\"synchronizing\"}} {}",
        s.ready_sessions, s.synchronizing_sessions
    )
    .expect("String output");
    out.push_str("# HELP rgg_delivery_records Retained delivery states; not cumulative or recipient acknowledgements.\n# TYPE rgg_delivery_records gauge\n");
    for (name, count) in [
        ("pending", s.deliveries.pending),
        ("leased", s.deliveries.leased),
        ("written_unconfirmed", s.deliveries.written_unconfirmed),
        ("expired", s.deliveries.expired),
        ("privacy_rejected", s.deliveries.privacy_rejected),
        ("rejected", s.deliveries.rejected),
    ] {
        writeln!(out, "rgg_delivery_records{{state=\"{name}\"}} {count}").expect("String output");
    }
    gauge(
        &mut out,
        "rgg_queued_payload_bytes",
        "Logical payload bytes of Pending and Leased records, not SQLite disk usage.",
        s.queued_payload_bytes,
    );
    gauge(
        &mut out,
        "rgg_oldest_queue_age_seconds",
        "Age of oldest Pending or Leased record; zero for empty or future queue.",
        s.oldest_queue_age_seconds,
    );
    gauge(
        &mut out,
        "rgg_contact_archive_bytes",
        "UTF-8 bytes in committed contact archives.",
        s.contact_archive_bytes,
    );
    gauge(
        &mut out,
        "rgg_unexpired_image_grants",
        "Grants before expiry; other authorization conditions still apply.",
        s.unexpired_image_grants,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgg_application::ports::StorageUnavailable;
    use std::sync::atomic::AtomicUsize;
    struct Repository {
        calls: AtomicUsize,
        fail: AtomicBool,
        slow: AtomicBool,
    }
    #[async_trait::async_trait]
    impl OperationalRepository for Repository {
        async fn operational_snapshot(&self) -> Result<OperationalSnapshot, StorageUnavailable> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.slow.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            if self.fail.load(Ordering::SeqCst) {
                Err(StorageUnavailable)
            } else {
                Ok(OperationalSnapshot::default())
            }
        }
    }
    fn setup() -> (Arc<Metrics>, Arc<Repository>) {
        let repository = Arc::new(Repository {
            calls: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
            slow: AtomicBool::new(false),
        });
        let state = Arc::new(Metrics {
            repository: repository.clone(),
            ready: Arc::new(AtomicBool::new(false)),
            authority: "127.0.0.1:9090".into(),
            origin: "http://127.0.0.1:9090".into(),
            started: Instant::now(),
            cache: Mutex::new(None),
            requests: Semaphore::new(2),
        });
        (state, repository)
    }
    #[tokio::test(start_paused = true)]
    async fn readiness_cache_expiry_and_storage_failure_never_export_an_expired_snapshot() {
        let (state, repository) = setup();
        assert_eq!(
            scrape(State(state.clone())).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(repository.calls.load(Ordering::SeqCst), 0);
        state.ready.store(true, Ordering::SeqCst);
        for _ in 0..3 {
            assert_eq!(scrape(State(state.clone())).await.status(), StatusCode::OK);
        }
        assert_eq!(repository.calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_millis(501)).await;
        repository.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            scrape(State(state.clone())).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(repository.calls.load(Ordering::SeqCst), 2);
        repository.fail.store(false, Ordering::SeqCst);
        assert_eq!(scrape(State(state.clone())).await.status(), StatusCode::OK);
        state.ready.store(false, Ordering::SeqCst);
        assert_eq!(
            scrape(State(state.clone())).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(repository.calls.load(Ordering::SeqCst), 3);
    }
    #[tokio::test(start_paused = true)]
    async fn busy_scrapes_and_a_stalled_repository_have_finite_admission_and_time() {
        let (state, repository) = setup();
        state.ready.store(true, Ordering::SeqCst);
        let permit = state.requests.acquire_many(2).await.unwrap();
        assert_eq!(
            scrape(State(state.clone())).await.status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(repository.calls.load(Ordering::SeqCst), 0);
        drop(permit);
        repository.slow.store(true, Ordering::SeqCst);
        let start = Instant::now();
        assert_eq!(
            scrape(State(state.clone())).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(start.elapsed(), Duration::from_secs(1));
        assert!(state.cache.lock().await.is_none());
        assert_eq!(state.requests.available_permits(), 2);
    }
}
