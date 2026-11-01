// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{
    HttpState,
    website::{PASSWORD_COOKIE, notice_at},
};
use axum::{
    Form,
    extract::{ConnectInfo, State, rejection::FormRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use rgg_application::{
    accounts::{AccountError, LegacyPassword},
    password::PasswordChange,
};
use rgg_domain::UserId;
use serde::Deserialize;
use std::net::SocketAddr;
use zeroize::Zeroizing;
fn notice(status: StatusCode, message: &str) -> Response {
    notice_at(status, message, "/password")
}
pub(crate) async fn form(
    State(state): State<HttpState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
) -> Response {
    let Some(web) = &state.website else {
        return notice(StatusCode::NOT_FOUND, "Zmiana hasła jest wyłączona.");
    };
    let token = match web.issue_password_token(remote.ip()) {
        Ok(value) => value,
        Err(status) => return notice(status, "Formularz jest chwilowo niedostępny."),
    };
    let mut response =
        Html(include_str!("password.html").replace("{{csrf}}", &token)).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{PASSWORD_COOKIE}={token}; Path=/password; HttpOnly; SameSite=Strict; Max-Age=600"
        ))
        .expect("fixed cookie and hex"),
    );
    response
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    uin: String,
    current_password: Zeroizing<String>,
    new_password: Zeroizing<String>,
    confirm_password: Zeroizing<String>,
    csrf: Zeroizing<String>,
}
fn command(input: Input) -> Result<PasswordChange, &'static str> {
    let uin = input.uin.trim();
    let user = uin
        .bytes()
        .all(|c| c.is_ascii_digit())
        .then(|| uin.parse::<u32>().ok())
        .flatten()
        .filter(|n| (1..=0x00ff_ffff).contains(n))
        .and_then(|n| UserId::new(n).ok())
        .ok_or("Podaj numer GG z zakresu 1–16777215.")?;
    let password = |value: &str| {
        LegacyPassword::new(Zeroizing::new(value.as_bytes().to_vec()))
            .map_err(|_| "Hasła muszą mieć 8–15 znaków ASCII, bez spacji.")
    };
    let current = password(&input.current_password)?;
    let replacement = password(&input.new_password)?;
    let confirmation = password(&input.confirm_password)?;
    if !replacement.matches(&confirmation) {
        return Err("Nowe hasła muszą być takie same.");
    }
    Ok(PasswordChange {
        user,
        current,
        replacement,
    })
}
pub(crate) async fn submit(
    State(state): State<HttpState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    input: Result<Form<Input>, FormRejection>,
) -> Response {
    let Some(web) = &state.website else {
        return notice(StatusCode::NOT_FOUND, "Zmiana hasła jest wyłączona.");
    };
    let input = match input {
        Ok(Form(input)) => input,
        Err(error) => {
            return notice(
                match error.status() {
                    StatusCode::PAYLOAD_TOO_LARGE => StatusCode::PAYLOAD_TOO_LARGE,
                    StatusCode::UNSUPPORTED_MEDIA_TYPE => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    _ => StatusCode::BAD_REQUEST,
                },
                "Nieprawidłowy formularz.",
            );
        }
    };
    if !web.consume_password_token(&headers, &input.csrf, remote.ip()) {
        return notice(
            StatusCode::FORBIDDEN,
            "Formularz wygasł lub został już użyty. Otwórz go ponownie.",
        );
    }
    let mut response = match command(input) {
        Err(message) => notice(StatusCode::BAD_REQUEST, message),
        Ok(input) => match web
            .passwords
            .as_ref()
            .expect("password guard")
            .change(input)
            .await
        {
            Ok(_) => notice(
                StatusCode::OK,
                "Hasło zmienione. Wpisz nowe hasło w kliencie GG i zaloguj się ponownie.",
            ),
            Err(
                AccountError::IncorrectPassword
                | AccountError::Inactive
                | AccountError::StaleCredential,
            ) => notice(
                StatusCode::FORBIDDEN,
                "Nie udało się potwierdzić danych. Sprawdź numer i obecne hasło lub spróbuj za 10 minut.",
            ),
            Err(AccountError::UnchangedPassword) => notice(
                StatusCode::BAD_REQUEST,
                "Nowe hasło musi być inne od obecnego.",
            ),
            Err(_) => notice(
                StatusCode::SERVICE_UNAVAILABLE,
                "Nie udało się potwierdzić zmiany. Sprawdź logowanie nowym hasłem przed ponowną próbą.",
            ),
        },
    };
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "rgg_password=; Path=/password; HttpOnly; SameSite=Strict; Max-Age=0",
        ),
    );
    response
}
