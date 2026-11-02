// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Explicit HTTPS origin and single local proxy trust boundary. Never infer a public URL.
use axum::http::{HeaderMap, header, uri::Authority};
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "ProxyInput")]
pub struct WebProxy {
    authority: String,
    origin: String,
    trusted_proxy: IpAddr,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProxyInput {
    origin: String,
    trusted_proxy: IpAddr,
}
impl TryFrom<ProxyInput> for WebProxy {
    type Error = &'static str;
    fn try_from(input: ProxyInput) -> Result<Self, Self::Error> {
        let invalid = "web_proxy requires a canonical HTTPS origin and one loopback proxy IP";
        if input.origin.len() > 280 || !input.trusted_proxy.is_loopback() {
            return Err(invalid);
        }
        let raw = input.origin.strip_prefix("https://").ok_or(invalid)?;
        let authority: Authority = raw.parse().map_err(|_| invalid)?;
        if authority.as_str() != raw || raw.contains('@') {
            return Err(invalid);
        }
        let host = authority.host();
        let unbracketed = host
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(host);
        if let Ok(ip) = unbracketed.parse::<IpAddr>() {
            let canonical = match ip {
                IpAddr::V4(_) => ip.to_string(),
                IpAddr::V6(_) => format!("[{ip}]"),
            };
            if host != canonical
                || ip.is_unspecified()
                || ip.is_multicast()
                || matches!(ip, IpAddr::V6(value) if value.to_ipv4_mapped().is_some())
            {
                return Err(invalid);
            }
        } else if host.len() > 253
            || !host.split('.').all(|part| {
                !part.is_empty()
                    && part.len() <= 63
                    && part.as_bytes()[0].is_ascii_alphanumeric()
                    && part.as_bytes()[part.len() - 1].is_ascii_alphanumeric()
                    && part
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            })
            || !host
                .rsplit('.')
                .next()
                .is_some_and(|tld| tld.bytes().all(|c| c.is_ascii_lowercase()))
        {
            // A letter-only final DNS label avoids browser reinterpretation of legacy numeric IPs.
            return Err(invalid);
        }
        let canonical = match authority.port_u16() {
            Some(0 | 443) => return Err(invalid),
            Some(port) => format!("{host}:{port}"),
            None => host.to_owned(),
        };
        if canonical != raw {
            return Err(invalid);
        }
        Ok(Self {
            authority: canonical,
            origin: input.origin,
            trusted_proxy: canonical_ip(input.trusted_proxy),
        })
    }
}

#[derive(Debug)]
pub struct WebEndpoint {
    authority: String,
    origin: String,
    proxy: Option<IpAddr>,
}
#[derive(Clone, Copy)]
pub(crate) struct WebsiteClient(pub IpAddr);

pub(crate) fn single(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        None
    } else {
        Some(value)
    }
}
fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(value) => value.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        _ => ip,
    }
}
impl WebEndpoint {
    pub fn new(bound: SocketAddr, proxy: Option<WebProxy>) -> Result<Self, &'static str> {
        if !bound.ip().is_loopback() || bound.port() == 0 {
            return Err("website requires a bound loopback listener");
        }
        Ok(match proxy {
            Some(proxy) => Self {
                authority: proxy.authority,
                origin: proxy.origin,
                proxy: Some(proxy.trusted_proxy),
            },
            None => Self {
                authority: bound.to_string(),
                origin: format!("http://{bound}"),
                proxy: None,
            },
        })
    }
    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }
    pub(crate) fn cookie_security(&self) -> &'static str {
        if self.proxy.is_some() { "; Secure" } else { "" }
    }
    pub(crate) fn client(
        &self,
        headers: &HeaderMap,
        remote: IpAddr,
        post: bool,
    ) -> Option<WebsiteClient> {
        let origin = single(headers, header::ORIGIN);
        if single(headers, header::HOST) != Some(self.authority.as_str())
            || (post || headers.contains_key(header::ORIGIN))
                && origin != Some(self.origin.as_str())
            || headers.contains_key("sec-fetch-site")
                && !matches!(
                    single(headers, header::HeaderName::from_static("sec-fetch-site")),
                    Some("same-origin" | "none")
                )
        {
            return None;
        }
        let ip = if let Some(proxy) = self.proxy {
            if canonical_ip(remote) != proxy
                || single(
                    headers,
                    header::HeaderName::from_static("x-forwarded-proto"),
                ) != Some("https")
            {
                return None;
            }
            single(headers, header::HeaderName::from_static("x-real-ip"))?
                .parse::<IpAddr>()
                .ok()?
        } else {
            // Forwarding headers never influence the ordinary direct local profile.
            if !remote.is_loopback() {
                return None;
            }
            remote
        };
        let ip = canonical_ip(ip);
        if ip.is_unspecified() || ip.is_multicast() {
            return None;
        }
        Some(WebsiteClient(ip))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    fn proxy(origin: &str) -> Result<WebProxy, &'static str> {
        ProxyInput {
            origin: origin.into(),
            trusted_proxy: "127.0.0.2".parse().unwrap(),
        }
        .try_into()
    }
    #[test]
    fn canonical_origin_has_no_implicit_aliases_paths_credentials_or_public_proxy() {
        for origin in [
            "https://rgg.test",
            "https://localhost:8443",
            "https://127.0.0.1",
            "https://[::1]:8443",
            "https://xn--test-rgg.example",
            "https://rgg.test:65535",
        ] {
            let parsed = proxy(origin).unwrap();
            assert_eq!(parsed.origin, origin);
            assert_eq!(parsed.authority, origin.strip_prefix("https://").unwrap());
        }
        for origin in [
            "",
            "http://rgg.test",
            "HTTPS://rgg.test",
            "https://RGG.test",
            "https://rgg.test/",
            "https://rgg.test/p",
            "https://rgg.test?x",
            "https://rgg.test#x",
            "https://rgg.test.",
            "https://a..test",
            "https://-a.test",
            "https://a-.test",
            "https://user@rgg.test",
            "https://rgg.test@attacker.test",
            "https://rgg.test:443",
            "https://rgg.test:0443",
            "https://rgg.test:0",
            "https://rgg.test:65536",
            "https://rgg.test:",
            "https://rgg.test:abc",
            "https://żółć.test",
            "https://rgg_test",
            "https://127.1",
            "https://2130706433",
            "https://0x7f000001",
            "https://0177.0.0.1",
            "https://0.0.0.0",
            "https://[::]",
            "https://[ff02::1]",
            "https://[0:0:0:0:0:0:0:1]",
            "https://[::1%25lo]",
            "https://[::ffff:192.0.2.4]",
            "https://rgg%2etest",
            "https://rgg.test\\evil",
            "https://rgg.test\n",
            " https://rgg.test",
        ] {
            assert!(proxy(origin).is_err(), "{origin:?}");
        }
        assert!(proxy(&format!("https://{}.test", "a".repeat(64))).is_err());
        for ip in ["0.0.0.0", "192.0.2.1", "10.0.0.1", "::", "2001:db8::1"] {
            assert!(
                WebProxy::try_from(ProxyInput {
                    origin: "https://rgg.test".into(),
                    trusted_proxy: ip.parse().unwrap()
                })
                .is_err()
            );
        }
        assert!(
            WebEndpoint::new(
                "0.0.0.0:8080".parse().unwrap(),
                Some(proxy("https://rgg.test").unwrap())
            )
            .is_err()
        );
        assert!(WebEndpoint::new("127.0.0.1:0".parse().unwrap(), None).is_err());
    }
    fn headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in [
            ("host", "rgg.test"),
            ("origin", "https://rgg.test"),
            ("x-real-ip", "192.0.2.4"),
            ("x-forwarded-proto", "https"),
            ("sec-fetch-site", "same-origin"),
        ] {
            headers.insert(
                header::HeaderName::from_static(name),
                HeaderValue::from_static(value),
            );
        }
        headers
    }
    #[test]
    fn only_one_explicit_proxy_and_unambiguous_headers_can_supply_identity() {
        let endpoint = WebEndpoint::new(
            "127.0.0.1:8080".parse().unwrap(),
            Some(proxy("https://rgg.test").unwrap()),
        )
        .unwrap();
        let peer = "127.0.0.2".parse().unwrap();
        let valid = headers();
        assert_eq!(
            endpoint.client(&valid, peer, true).unwrap().0,
            "192.0.2.4".parse::<IpAddr>().unwrap()
        );
        assert_eq!(endpoint.cookie_security(), "; Secure");
        for remote in ["127.0.0.1", "127.0.0.3", "::1", "192.0.2.4"] {
            assert!(
                endpoint
                    .client(&valid, remote.parse().unwrap(), false)
                    .is_none()
            );
        }
        for name in [
            "host",
            "origin",
            "x-real-ip",
            "x-forwarded-proto",
            "sec-fetch-site",
        ] {
            let mut bad = valid.clone();
            bad.append(
                header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                bad[name].clone(),
            );
            assert!(endpoint.client(&bad, peer, true).is_none(), "{name}");
            bad.remove(name);
            if name != "sec-fetch-site" {
                assert!(endpoint.client(&bad, peer, true).is_none(), "{name}");
            }
        }
        for (name, value) in [
            ("host", "attacker.test"),
            ("origin", "http://rgg.test"),
            ("origin", "null"),
            ("origin", "https://rgg.test https://other.test"),
            ("x-forwarded-proto", "http"),
            ("x-forwarded-proto", "https,http"),
            ("sec-fetch-site", "cross-site"),
            ("sec-fetch-site", "same-site"),
            ("x-real-ip", "192.0.2.4,192.0.2.5"),
            ("x-real-ip", "192.0.2.4:8080"),
            ("x-real-ip", "[::1]"),
            ("x-real-ip", "unknown"),
            ("x-real-ip", "0.0.0.0"),
            ("x-real-ip", "ff02::1"),
        ] {
            let mut bad = valid.clone();
            bad.insert(
                header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_static(value),
            );
            assert!(
                endpoint.client(&bad, peer, true).is_none(),
                "{name}={value}"
            );
        }
        let mut get = valid.clone();
        get.remove("origin");
        assert!(endpoint.client(&get, peer, false).is_some());
        get.insert("origin", HeaderValue::from_static("https://attacker.test"));
        assert!(endpoint.client(&get, peer, false).is_none());
        let mut aliases = valid;
        aliases.insert("x-real-ip", HeaderValue::from_static("::ffff:192.0.2.4"));
        aliases.insert(
            "forwarded",
            HeaderValue::from_static("for=203.0.113.3;proto=http;host=attacker.test"),
        );
        aliases.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.3"));
        aliases.insert(
            "x-forwarded-host",
            HeaderValue::from_static("attacker.test"),
        );
        assert_eq!(
            endpoint.client(&aliases, peer, true).unwrap().0,
            "192.0.2.4".parse::<IpAddr>().unwrap()
        );
    }
    #[test]
    fn direct_profile_ignores_forwarding_headers_and_preserves_actual_peer() {
        let endpoint = WebEndpoint::new("127.0.0.1:8080".parse().unwrap(), None).unwrap();
        let mut headers = headers();
        headers.insert("host", HeaderValue::from_static("127.0.0.1:8080"));
        headers.insert("origin", HeaderValue::from_static("http://127.0.0.1:8080"));
        let peer = "127.0.0.3".parse().unwrap();
        assert_eq!(endpoint.client(&headers, peer, true).unwrap().0, peer);
        assert_eq!(endpoint.cookie_security(), "");
        assert!(
            endpoint
                .client(&headers, "192.0.2.4".parse().unwrap(), true)
                .is_none()
        );
    }
}
