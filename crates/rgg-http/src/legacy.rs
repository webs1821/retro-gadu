// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// HTTP wire facts: libgadu-wire-reference; no upstream code copied.
use crate::HttpState;
use axum::{
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use std::{
    net::{SocketAddr, SocketAddrV4},
    sync::atomic::Ordering,
};

/// Operator-selected destination for clients; never a listener or a request-derived value.
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(try_from = "SocketAddrV4")]
pub struct AdvertisedEndpoint(SocketAddrV4);
impl TryFrom<SocketAddrV4> for AdvertisedEndpoint {
    type Error = &'static str;
    fn try_from(value: SocketAddrV4) -> Result<Self, Self::Error> {
        let first = value.ip().octets()[0];
        if value.port() == 0 || first == 0 || first >= 224 {
            return Err(
                "advertised GG endpoint requires a unicast IPv4 address and a nonzero port",
            );
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Discovery(SocketAddr);
impl Discovery {
    pub fn new(
        endpoint: SocketAddr,
        advertised: Option<AdvertisedEndpoint>,
    ) -> Result<Self, &'static str> {
        if !endpoint.is_ipv4() || !endpoint.ip().is_loopback() || endpoint.port() == 0 {
            return Err("legacy discovery requires a bound local IPv4 endpoint");
        }
        Ok(Self(advertised.map_or(endpoint, |value| value.0.into())))
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
    fn discovery_requires_a_bound_local_listener_even_with_an_explicit_destination() {
        for value in [
            "0.0.0.0:8074",
            "192.168.1.1:8074",
            "[::1]:8074",
            "127.0.0.1:0",
        ] {
            assert!(Discovery::new(value.parse().unwrap(), None).is_err());
            let advertised =
                AdvertisedEndpoint::try_from("198.51.100.7:18074".parse::<SocketAddrV4>().unwrap())
                    .unwrap();
            assert!(Discovery::new(value.parse().unwrap(), Some(advertised)).is_err());
        }
        assert!(Discovery::new("127.0.0.1:8074".parse().unwrap(), None).is_ok());
        let endpoint = Discovery::new("127.0.0.1:8074".parse().unwrap(), None).unwrap();
        assert_eq!(
            address_line(Some(endpoint), true),
            "0 0 127.0.0.1:8074 127.0.0.1\r\n"
        );
        assert_eq!(
            address_line(Some(endpoint), false),
            address_line(None, true)
        );
    }
    #[test]
    fn explicit_destination_changes_only_the_ready_address_line_and_is_bounded() {
        for value in [
            "0.0.0.0:8074",
            "0.1.2.3:8074",
            "224.0.0.1:8074",
            "240.0.0.1:8074",
            "255.255.255.255:8074",
            "127.0.0.1:0",
        ] {
            assert!(AdvertisedEndpoint::try_from(value.parse::<SocketAddrV4>().unwrap()).is_err());
        }
        for value in ["127.0.0.2:18074", "192.168.1.2:8074", "198.51.100.7:65535"] {
            let destination = value.parse::<SocketAddrV4>().unwrap();
            let advertised = AdvertisedEndpoint::try_from(destination).unwrap();
            let discovery =
                Discovery::new("127.0.0.1:8074".parse().unwrap(), Some(advertised)).unwrap();
            assert_eq!(
                address_line(Some(discovery), true),
                format!("0 0 {destination} {}\r\n", destination.ip())
            );
            assert_eq!(
                address_line(Some(discovery), false),
                "0 0 notoperating notoperating\r\n"
            );
        }
    }
}
