// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::HttpState;
use axum::{
    Form, Router,
    extract::{ConnectInfo, DefaultBodyLimit, Request, State, rejection::FormRejection},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use rand::TryRngCore;
use rgg_application::{
    accounts::{AccountError, LegacyPassword},
    registration::{NewAccount, RegistrationService},
};
use rgg_domain::UserId;
use serde::Deserialize;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;
use zeroize::Zeroizing;

const COOKIE: &str = "rgg_register";
// Capacity exceeds all admissions possible during the ten-minute retention window.
const MAX_TOKENS: usize = 2048;
const MAX_SOURCE_IPS: usize = 2048;
struct Budget {
    since: Instant,
    forms: u16,
    posts: u16,
}
impl Budget {
    fn new(now: Instant) -> Self {
        Self {
            since: now,
            forms: 0,
            posts: 0,
        }
    }
    fn refresh(&mut self, now: Instant) {
        if now.saturating_duration_since(self.since) >= Duration::from_secs(60) {
            *self = Self::new(now);
        }
    }
}
struct Gate {
    tokens: HashMap<[u8; 32], (IpAddr, Instant)>,
    ips: HashMap<IpAddr, Budget>,
    global: Budget,
}
impl Gate {
    fn new(now: Instant) -> Self {
        Self {
            tokens: HashMap::new(),
            ips: HashMap::new(),
            global: Budget::new(now),
        }
    }
    fn purge(&mut self, now: Instant) {
        self.tokens.retain(|_, (_, expires)| *expires > now);
        self.ips
            .retain(|_, b| now.saturating_duration_since(b.since) < Duration::from_secs(600));
    }
    fn charge(&mut self, ip: IpAddr, post: bool, now: Instant) -> Result<(), StatusCode> {
        self.purge(now);
        self.global.refresh(now);
        if !self.ips.contains_key(&ip) && self.ips.len() >= MAX_SOURCE_IPS {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        let budget = self.ips.entry(ip).or_insert_with(|| Budget::new(now));
        budget.refresh(now);
        let (local, global, local_max, global_max) = if post {
            (&mut budget.posts, &mut self.global.posts, 20, 60)
        } else {
            (&mut budget.forms, &mut self.global.forms, 60, 120)
        };
        if *local >= local_max || *global >= global_max {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        *local += 1;
        *global += 1;
        Ok(())
    }
    fn issue(&mut self, token: [u8; 32], ip: IpAddr, now: Instant) -> Result<(), StatusCode> {
        self.purge(now);
        if self.tokens.len() >= MAX_TOKENS || self.tokens.contains_key(&token) {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        self.tokens
            .insert(token, (ip, now + Duration::from_secs(600)));
        Ok(())
    }
    fn consume(&mut self, token: [u8; 32], ip: IpAddr, now: Instant) -> bool {
        self.purge(now);
        if !self
            .tokens
            .get(&token)
            .is_some_and(|(owner, expires)| *owner == ip && *expires > now)
        {
            return false;
        }
        self.tokens.remove(&token);
        true
    }
}
pub(crate) struct Website {
    service: Arc<RegistrationService>,
    authority: String,
    origin: String,
    gate: Mutex<Gate>,
    concurrent: Semaphore,
}
impl Website {
    pub fn new(
        service: Arc<RegistrationService>,
        address: SocketAddr,
    ) -> Result<Self, &'static str> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err("registration website requires a bound loopback listener");
        }
        Ok(Self {
            service,
            authority: address.to_string(),
            origin: format!("http://{address}"),
            gate: Mutex::new(Gate::new(Instant::now())),
            concurrent: Semaphore::new(8),
        })
    }
    fn accepts_headers(&self, headers: &HeaderMap, post: bool) -> bool {
        single_header(headers, header::HOST) == Some(self.authority.as_str())
            && (!post || single_header(headers, header::ORIGIN) == Some(self.origin.as_str()))
            && headers
                .get("sec-fetch-site")
                .is_none_or(|v| v == "same-origin" || v == "none")
    }
}
fn single_header(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    let values = headers.get_all(name);
    let mut iter = values.iter();
    let value = iter.next()?.to_str().ok()?;
    if iter.next().is_some() {
        None
    } else {
        Some(value)
    }
}
fn secured(mut response: Response) -> Response {
    for (name, value) in [
        (header::CACHE_CONTROL, "no-store"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (header::REFERRER_POLICY, "no-referrer"),
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; style-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
        ),
    ] {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    response
}
fn notice(status: StatusCode, message: &str) -> Response {
    // Callers supply only fixed messages or a validated numeric UIN, never raw input/errors.
    secured((status,Html(format!("<!doctype html><html lang=\"pl\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Retro Gadu</title><link rel=\"stylesheet\" href=\"/assets/registration.css\"><main><p class=\"brand\">Retro Gadu <span>rGG</span></p><h1>{message}</h1><p><a href=\"/register\">Wróć do formularza</a></p></main></html>"))).into_response())
}
pub(crate) fn routes(state: HttpState) -> Router<HttpState> {
    Router::new()
        .route("/register", get(form).post(submit))
        .route("/assets/registration.css", get(styles))
        .layer(DefaultBodyLimit::max(4096))
        .route_layer(middleware::from_fn_with_state(state, guard))
}
async fn guard(
    State(state): State<HttpState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let Some(web) = &state.registration else {
        return notice(StatusCode::NOT_FOUND, "Rejestracja WWW jest wyłączona.");
    };
    if !state.ready.load(Ordering::Acquire) {
        return notice(
            StatusCode::SERVICE_UNAVAILABLE,
            "Serwer nie jest jeszcze gotowy.",
        );
    }
    let post = request.method() == Method::POST;
    if !web.accepts_headers(request.headers(), post) {
        return notice(
            StatusCode::FORBIDDEN,
            "Otwórz formularz pod lokalnym adresem tego serwera.",
        );
    }
    let charged = web
        .gate
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
        .and_then(|mut g| g.charge(remote.ip(), post, Instant::now()));
    if let Err(status) = charged {
        return notice(status, "Zbyt wiele prób. Spróbuj ponownie za minutę.");
    }
    let Ok(_permit) = web.concurrent.try_acquire() else {
        return notice(
            StatusCode::TOO_MANY_REQUESTS,
            "Serwer jest zajęty. Spróbuj ponownie.",
        );
    };
    match tokio::time::timeout(Duration::from_secs(5), next.run(request)).await {
        Ok(response) => secured(response),
        Err(_) => notice(
            StatusCode::REQUEST_TIMEOUT,
            "Przekroczono czas obsługi formularza.",
        ),
    }
}
async fn styles() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("registration.css"),
    )
        .into_response()
}
async fn form(
    State(state): State<HttpState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
) -> Response {
    let Some(web) = &state.registration else {
        return notice(StatusCode::NOT_FOUND, "Rejestracja jest wyłączona.");
    };
    let mut token = [0u8; 32];
    if rand::rngs::OsRng.try_fill_bytes(&mut token).is_err() {
        return notice(
            StatusCode::SERVICE_UNAVAILABLE,
            "Formularz jest chwilowo niedostępny.",
        );
    }
    let result = web
        .gate
        .lock()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
        .and_then(|mut g| g.issue(token, remote.ip(), Instant::now()));
    if let Err(status) = result {
        return notice(status, "Formularz jest chwilowo niedostępny.");
    }
    let token = hex::encode(token);
    let mut response = secured(
        Html(include_str!("registration.html").replace("{{csrf}}", &token)).into_response(),
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{COOKIE}={token}; Path=/register; HttpOnly; SameSite=Strict; Max-Age=600"
        ))
        .expect("fixed cookie syntax and hex token"),
    );
    response
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationForm {
    requested_uin: String,
    invitation: Zeroizing<String>,
    password: Zeroizing<String>,
    email: String,
    csrf: Zeroizing<String>,
}
fn token(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut decoded = [0u8; 32];
    hex::decode_to_slice(value, &mut decoded).ok()?;
    Some(decoded)
}
fn cookie(headers: &HeaderMap) -> Option<[u8; 32]> {
    let mut found = None;
    for header in headers.get_all(header::COOKIE) {
        for field in header.to_str().ok()?.split(';') {
            let Some((name, value)) = field.trim().split_once('=') else {
                continue;
            };
            if name == COOKIE {
                if found.is_some() {
                    return None;
                }
                found = Some(token(value)?);
            }
        }
    }
    found
}
fn account_input(form: RegistrationForm) -> Result<NewAccount, AccountError> {
    let value = form.requested_uin.trim();
    let requested_uin = if value.is_empty() {
        None
    } else {
        if !value.bytes().all(|c| c.is_ascii_digit()) {
            return Err(AccountError::UnsupportedNumber);
        }
        Some(
            UserId::new(
                value
                    .parse::<u32>()
                    .map_err(|_| AccountError::UnsupportedNumber)?,
            )
            .map_err(|_| AccountError::UnsupportedNumber)?,
        )
    };
    Ok(NewAccount {
        requested_uin,
        invitation: form.invitation,
        password: LegacyPassword::new(Zeroizing::new(form.password.as_bytes().to_vec()))?,
        email: form.email.trim().to_owned(),
    })
}
async fn submit(
    State(state): State<HttpState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    input: Result<Form<RegistrationForm>, FormRejection>,
) -> Response {
    let Some(web) = &state.registration else {
        return notice(StatusCode::NOT_FOUND, "Rejestracja jest wyłączona.");
    };
    let form = match input {
        Ok(Form(form)) => form,
        Err(rejection) => {
            let status = match rejection.status() {
                StatusCode::PAYLOAD_TOO_LARGE => StatusCode::PAYLOAD_TOO_LARGE,
                StatusCode::UNSUPPORTED_MEDIA_TYPE => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                _ => StatusCode::BAD_REQUEST,
            };
            return notice(status, "Nieprawidłowy formularz.");
        }
    };
    let valid = match (cookie(&headers), token(&form.csrf)) {
        (Some(cookie), Some(submitted)) if cookie.ct_eq(&submitted).unwrap_u8() == 1 => web
            .gate
            .lock()
            .map(|mut g| g.consume(cookie, remote.ip(), Instant::now()))
            .unwrap_or(false),
        _ => false,
    };
    if !valid {
        return notice(
            StatusCode::FORBIDDEN,
            "Formularz wygasł lub został już użyty. Otwórz go ponownie.",
        );
    }
    let result = match account_input(form) {
        Ok(input) => web.service.register(input).await,
        Err(error) => Err(error),
    };
    let mut response = match result {
        Ok(user) => notice(
            StatusCode::CREATED,
            &format!(
                "Konto gotowe. Twój numer GG: <strong data-uin=\"{}\">{}</strong>",
                user.get(),
                user.get()
            ),
        ),
        Err(AccountError::InvalidPassword) => notice(
            StatusCode::BAD_REQUEST,
            "Hasło musi mieć 8–15 znaków ASCII, bez spacji.",
        ),
        Err(AccountError::InvalidEmail) => {
            notice(StatusCode::BAD_REQUEST, "Podaj poprawny adres e-mail.")
        }
        Err(AccountError::UnsupportedNumber) => notice(
            StatusCode::BAD_REQUEST,
            "Numer musi mieścić się w zakresie 1–16777215.",
        ),
        Err(AccountError::NumberTaken) => notice(
            StatusCode::CONFLICT,
            "Ten numer jest już zajęty lub zarezerwowany.",
        ),
        Err(AccountError::InvalidInvite) => notice(
            StatusCode::BAD_REQUEST,
            "Kod zaproszenia jest nieprawidłowy lub został wykorzystany.",
        ),
        Err(_) => notice(
            StatusCode::SERVICE_UNAVAILABLE,
            "Nie udało się zakończyć rejestracji. Spróbuj ponownie później.",
        ),
    };
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "rgg_register=; Path=/register; HttpOnly; SameSite=Strict; Max-Age=0",
        ),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csrf_is_single_use_ip_bound_expiring_and_bounded() {
        let now = Instant::now();
        let ip = "127.0.0.1".parse().unwrap();
        let other = "127.0.0.2".parse().unwrap();
        let mut gate = Gate::new(now);
        gate.issue([1; 32], ip, now).unwrap();
        assert!(!gate.consume([1; 32], other, now));
        assert!(gate.consume([1; 32], ip, now));
        assert!(!gate.consume([1; 32], ip, now));
        gate.issue([2; 32], ip, now).unwrap();
        assert!(!gate.consume([2; 32], ip, now + Duration::from_secs(600)));
        for n in 0..MAX_TOKENS {
            let mut t = [0; 32];
            t[..2].copy_from_slice(&(n as u16).to_le_bytes());
            gate.issue(t, ip, now).unwrap();
        }
        assert!(gate.issue([255; 32], ip, now).is_err());
        assert!(
            gate.issue([255; 32], ip, now + Duration::from_secs(600))
                .is_ok()
        );
    }
    #[test]
    fn budgets_and_ambiguous_cookies_are_rejected() {
        let now = Instant::now();
        let ip = "127.0.0.1".parse().unwrap();
        let mut gate = Gate::new(now);
        for _ in 0..20 {
            gate.charge(ip, true, now).unwrap();
        }
        assert_eq!(
            gate.charge(ip, true, now),
            Err(StatusCode::TOO_MANY_REQUESTS)
        );
        assert!(gate.charge(ip, true, now + Duration::from_secs(60)).is_ok());
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{COOKIE}={}", "ab".repeat(32))).unwrap(),
        );
        assert_eq!(cookie(&headers), Some([0xab; 32]));
        headers.append(header::COOKIE, headers[header::COOKIE].clone());
        assert!(cookie(&headers).is_none());
    }
}
