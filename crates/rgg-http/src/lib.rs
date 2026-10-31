// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rgg_application::ports::HealthRepository;
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone)]
pub struct HttpState {
    repository: Arc<dyn HealthRepository>,
    ready: Arc<AtomicBool>,
}
impl HttpState {
    pub fn new(repository: Arc<dyn HealthRepository>) -> Self {
        Self {
            repository,
            ready: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn set_ready(&self, value: bool) {
        self.ready.store(value, Ordering::Release);
    }
}
#[derive(Serialize)]
struct Health {
    service: &'static str,
    status: &'static str,
}
pub fn router(state: HttpState) -> Router {
    Router::new()
        .route(
            "/health/live",
            get(|| async {
                Json(Health {
                    service: "rGG",
                    status: "live",
                })
            }),
        )
        .route("/health/ready", get(ready))
        .with_state(state)
}
async fn ready(State(state): State<HttpState>) -> Response {
    let healthy = state.ready.load(Ordering::Acquire)
        && matches!(
            tokio::time::timeout(Duration::from_secs(2), state.repository.check()).await,
            Ok(Ok(()))
        )
        && state.ready.load(Ordering::Acquire);
    let (status, text) = if healthy {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not_ready")
    };
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(Health {
            service: "rGG",
            status: text,
        }),
    )
        .into_response()
}
