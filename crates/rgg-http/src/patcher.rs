// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::HttpState;
use axum::{
    extract::State,
    http::header,
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub default_host: String,
}
impl Settings {
    pub fn valid(&self) -> bool {
        let host = self.default_host.as_bytes();
        (1..=16).contains(&host.len())
            && host.first().is_some_and(u8::is_ascii_alphanumeric)
            && host.last().is_some_and(u8::is_ascii_alphanumeric)
            && host
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-'))
            && host
                .windows(2)
                .all(|pair| pair[0].is_ascii_alphanumeric() || pair[1].is_ascii_alphanumeric())
    }
}
pub(crate) const CSP: &str = "default-src 'none'; script-src 'self'; connect-src 'self'; style-src 'self'; form-action 'none'; frame-ancestors 'none'; base-uri 'none'";
pub(crate) async fn page(State(state): State<HttpState>) -> Response {
    let Some(settings) = state.website.as_ref().and_then(|w| w.patcher.as_ref()) else {
        return crate::website::notice(
            axum::http::StatusCode::NOT_FOUND,
            "Ta strona jest wyłączona.",
        );
    };
    // The settings validator admits only ASCII alphanumeric characters, dots and hyphens.
    Html(include_str!("patcher.html").replace("{{default_host}}", &settings.default_host))
        .into_response()
}
pub(crate) async fn ui() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("client-patcher.mjs"),
    )
}
pub(crate) async fn core() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("client-patcher-core.mjs"),
    )
}
pub(crate) async fn plan() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        include_str!("client-patch-plan.json"),
    )
}
