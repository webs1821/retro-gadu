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
use rgg_application::images::{ImageObservations, ImageSnapshot};
use rgg_application::operations::{OperationalRepository, OperationalSnapshot};
use rgg_application::telemetry::RuntimeTelemetry;
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
    telemetry: Arc<RuntimeTelemetry>,
    images: Option<Arc<dyn ImageObservations>>,
    ready: Arc<AtomicBool>,
    authority: String,
    origin: String,
    started: Instant,
    cache: Mutex<Option<(Instant, OperationalSnapshot)>>,
    requests: Semaphore,
}
pub fn router(
    repository: Arc<dyn OperationalRepository>,
    telemetry: Arc<RuntimeTelemetry>,
    images: Option<Arc<dyn ImageObservations>>,
    health: &HttpState,
    address: SocketAddr,
) -> Result<Router, &'static str> {
    if !address.ip().is_loopback() || address.port() == 0 {
        return Err("metrics requires a bound loopback listener");
    }
    let state = Arc::new(Metrics {
        repository,
        telemetry,
        images,
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
            render(
                sample,
                state.started.elapsed().as_secs_f64(),
                state.telemetry.snapshot(),
                state
                    .images
                    .as_ref()
                    .and_then(|images| images.image_snapshot()),
            ),
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
fn render(
    s: OperationalSnapshot,
    uptime: f64,
    runtime: rgg_application::telemetry::Snapshot,
    images: Option<ImageSnapshot>,
) -> String {
    let mut out = String::with_capacity(16384);
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
    crate::metrics_runtime::append(&mut out, runtime);
    crate::metrics_images::append(&mut out, images, std::time::Instant::now());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgg_application::ports::StorageUnavailable;
    use std::sync::atomic::AtomicUsize;
    #[test]
    fn all_families_at_extreme_values_fit_the_response_budget() {
        use rgg_application::{
            operations::DeliveryRecords,
            telemetry::{
                Admission, ConnectionEnd, Histogram, Operation, Outcome, ProcessSample, Snapshot,
            },
        };
        let now = std::time::Instant::now();
        let s = OperationalSnapshot {
            schema_version: u64::MAX,
            sampled_at: i64::MAX,
            active_accounts: u64::MAX,
            disabled_accounts: u64::MAX,
            retired_numbers: u64::MAX,
            ready_sessions: u64::MAX,
            synchronizing_sessions: u64::MAX,
            deliveries: DeliveryRecords {
                pending: u64::MAX,
                leased: u64::MAX,
                written_unconfirmed: u64::MAX,
                expired: u64::MAX,
                privacy_rejected: u64::MAX,
                rejected: u64::MAX,
            },
            queued_payload_bytes: u64::MAX,
            oldest_queue_age_seconds: u64::MAX,
            contact_archive_bytes: u64::MAX,
            unexpired_image_grants: u64::MAX,
        };
        let runtime = Snapshot {
            process: Some(ProcessSample {
                observed_at: now,
                cpu_time: Some(Duration::MAX),
                resident_bytes: Some(u64::MAX),
            }),
            active_connections: u64::MAX,
            admissions: [u64::MAX; Admission::ALL.len()],
            endings: [u64::MAX; ConnectionEnd::ALL.len()],
            outcomes: [[u64::MAX; Outcome::ALL.len()]; Operation::ALL.len()],
            latency: [Histogram {
                buckets: [u64::MAX; 10],
                sum_nanos: u128::MAX,
            }; 3],
        };
        let images = ImageSnapshot {
            transfers: [u64::MAX; 4],
            reserved_bytes: u64::MAX,
            buffered_bytes: u64::MAX,
            buffer_capacity_bytes: u64::MAX,
            oldest_started: Some(now),
        };
        let out = render(s, Duration::MAX.as_secs_f64(), runtime, Some(images));
        assert!(out.len() < 16384, "{}", out.len());
        assert_eq!(out.matches("# TYPE ").count(), 27);
        assert_eq!(
            out.lines().filter(|line| !line.starts_with('#')).count(),
            126
        );
        assert!(out.is_ascii() && !out.contains("NaN") && !out.contains(" Inf"));
    }
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
            telemetry: Arc::new(RuntimeTelemetry::default()),
            images: None,
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
