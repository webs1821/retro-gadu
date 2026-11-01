// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
pub mod downloads;
mod legacy;
pub mod metrics;
mod password_change;
pub mod patcher;
mod statistics;
mod website;
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, get},
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
    discovery: Option<legacy::Discovery>,
    website: Option<Arc<website::Website>>,
}
impl HttpState {
    pub fn new(repository: Arc<dyn HealthRepository>) -> Self {
        Self {
            repository,
            ready: Arc::new(AtomicBool::new(false)),
            discovery: None,
            website: None,
        }
    }
    pub fn set_ready(&self, value: bool) {
        self.ready.store(value, Ordering::Release);
    }
    pub fn with_website(
        mut self,
        service: Option<Arc<rgg_application::registration::RegistrationService>>,
        statistics: Option<Arc<dyn rgg_application::statistics::StatisticsRepository>>,
        downloads: Option<Arc<downloads::Catalog>>,
        passwords: Option<Arc<rgg_application::password::PasswordService>>,
        patcher: Option<patcher::Settings>,
        address: std::net::SocketAddr,
    ) -> Result<Self, &'static str> {
        self.website = Some(Arc::new(website::Website::new(
            service, statistics, downloads, passwords, patcher, address,
        )?));
        Ok(self)
    }
    pub fn with_legacy_discovery(
        mut self,
        endpoint: std::net::SocketAddr,
    ) -> Result<Self, &'static str> {
        self.discovery = Some(legacy::Discovery::new(endpoint)?);
        Ok(self)
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
        .route("/appsvc/appmsg3.asp", get(legacy::discovery))
        .route("/appsvc/appmsg4.asp", get(legacy::discovery))
        .route(
            "/appsvc/fmregister3.asp",
            any(legacy::deny_account_operation),
        )
        .route("/fmregister.php", any(legacy::deny_account_operation))
        .route("/fmsendpwd.php", any(legacy::deny_account_operation))
        .merge(website::routes(state.clone()))
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
