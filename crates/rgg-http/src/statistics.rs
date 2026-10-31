// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{HttpState, website::notice};
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use rgg_application::statistics::PublicStatistics;
use serde::Serialize;

#[derive(Serialize)]
struct Counters {
    registered: u64,
    publicly_online: u64,
    sampled_at: i64,
}
#[derive(Serialize)]
struct Failure {
    error: &'static str,
}
impl From<PublicStatistics> for Counters {
    fn from(s: PublicStatistics) -> Self {
        Self {
            registered: s.registered,
            publicly_online: s.publicly_online,
            sampled_at: s.sampled_at,
        }
    }
}
async fn snapshot(state: &HttpState) -> Result<Counters, (StatusCode, Json<Failure>)> {
    let Some(repository) = state
        .website
        .as_ref()
        .and_then(|web| web.statistics.as_ref())
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(Failure {
                error: "statistics_disabled",
            }),
        ));
    };
    repository
        .public_statistics()
        .await
        .map(Into::into)
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Failure {
                    error: "statistics_unavailable",
                }),
            )
        })
}
pub(crate) async fn api(State(state): State<HttpState>) -> Response {
    match snapshot(&state).await {
        Ok(s) => Json(s).into_response(),
        Err(response) => response.into_response(),
    }
}
pub(crate) async fn home(State(state): State<HttpState>) -> Response {
    let Ok(s) = snapshot(&state).await else {
        return notice(
            StatusCode::SERVICE_UNAVAILABLE,
            "Statystyki są chwilowo niedostępne.",
        );
    };
    let registration = if state
        .website
        .as_ref()
        .is_some_and(|web| web.service.is_some())
    {
        "<a class=\"button\" href=\"/register\">Załóż konto</a>"
    } else {
        ""
    };
    Html(
        include_str!("home.html")
            .replace("{{registered}}", &s.registered.to_string())
            .replace("{{online}}", &s.publicly_online.to_string())
            .replace("{{registration_link}}", registration),
    )
    .into_response()
}
