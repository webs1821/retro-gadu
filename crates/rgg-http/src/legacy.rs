// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// HTTP wire facts: libgadu-wire-reference; no upstream code copied.
use crate::HttpState;
use axum::{
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use std::{net::SocketAddr, sync::atomic::Ordering};

#[derive(Clone, Copy)]
pub(crate) struct Discovery(SocketAddr);
impl Discovery {
    pub fn new(endpoint: SocketAddr) -> Result<Self, &'static str> {
        if !endpoint.is_ipv4() || !endpoint.ip().is_loopback() || endpoint.port() == 0 {
            return Err("legacy discovery requires a bound local IPv4 endpoint");
        }
        Ok(Self(endpoint))
    }
}
fn response(status: StatusCode, body: String) -> Response {
    (
        status,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::CONNECTION, "close"),
            (header::CONTENT_TYPE, "text/plain; charset=us-ascii"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        body,
    )
        .into_response()
}
pub(crate) async fn discovery(State(state): State<HttpState>) -> Response {
    // Request Host, forwarded headers, query UIN and version never influence the address.
    response(
        StatusCode::OK,
        address_line(state.discovery, state.ready.load(Ordering::Acquire)),
    )
}
fn address_line(discovery: Option<Discovery>, ready: bool) -> String {
    match discovery.filter(|_| ready) {
        Some(Discovery(endpoint)) => format!("0 0 {} {}\r\n", endpoint, endpoint.ip()),
        None => "0 0 notoperating notoperating\r\n".into(),
    }
}
pub(crate) async fn deny_account_operation() -> Response {
    // Deliberately does not deserialize a body or call any account/invitation port.
    response(
        StatusCode::FORBIDDEN,
        "legacy_account_operations_disabled\r\n".into(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_cannot_advertise_a_remote_ipv6_or_unbound_endpoint() {
        for value in [
            "0.0.0.0:8074",
            "192.168.1.1:8074",
            "[::1]:8074",
            "127.0.0.1:0",
        ] {
            assert!(Discovery::new(value.parse().unwrap()).is_err());
        }
        assert!(Discovery::new("127.0.0.1:8074".parse().unwrap()).is_ok());
        let endpoint = Discovery::new("127.0.0.1:8074".parse().unwrap()).unwrap();
        assert_eq!(
            address_line(Some(endpoint), true),
            "0 0 127.0.0.1:8074 127.0.0.1\r\n"
        );
        assert_eq!(
            address_line(Some(endpoint), false),
            address_line(None, true)
        );
    }
}
