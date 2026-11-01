// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Operator-supplied artifacts: verified immutable snapshots, separate from the AGPL source.
use crate::{HttpState, website::notice};
use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path as FsPath, sync::Arc};
use tokio::sync::Semaphore;
const MAX_MANIFEST: u64 = 16 * 1024;
const MAX_FILE: u64 = 32 * 1024 * 1024;
const MAX_TOTAL: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u8,
    artifacts: Vec<Artifact>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Client,
    Installer,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    id: String,
    kind: Kind,
    title: String,
    file: String,
    size: u64,
    sha256: String,
    source_url: String,
    license: String,
    instructions: String,
}
pub struct Catalog {
    items: Vec<(Artifact, Bytes)>,
    streams: Arc<Semaphore>,
}
fn token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn filename(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(|c| c.is_control())
}
fn source_url(value: &str) -> bool {
    value.len() <= 512
        && value.starts_with("https://")
        && value.len() > 8
        && value.is_ascii()
        && value.parse::<axum::http::Uri>().is_ok_and(|uri| {
            uri.scheme_str() == Some("https") && uri.host().is_some_and(|h| !h.is_empty())
        })
        && !value
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control() || b == b'\\' || b == b'@')
}
fn regular(path: &FsPath, max: u64) -> Result<Vec<u8>, &'static str> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| "artifact unavailable")?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > max {
        return Err("artifact is not a bounded regular file");
    }
    let file = std::fs::File::open(path).map_err(|_| "artifact unavailable")?;
    if !file
        .metadata()
        .map_err(|_| "artifact unavailable")?
        .is_file()
    {
        return Err("artifact is not a regular file");
    }
    let mut data = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut data)
        .map_err(|_| "artifact unavailable")?;
    if data.len() as u64 > max {
        return Err("artifact too large");
    }
    Ok(data)
}
impl Catalog {
    pub fn load(path: &FsPath) -> Result<Self, &'static str> {
        let bytes = regular(path, MAX_MANIFEST)?;
        let manifest: Manifest =
            toml::from_str(std::str::from_utf8(&bytes).map_err(|_| "invalid artifact manifest")?)
                .map_err(|_| "invalid artifact manifest")?;
        if manifest.schema != 1 || !(1..=4).contains(&manifest.artifacts.len()) {
            return Err("invalid artifact manifest");
        }
        let root = path.parent().ok_or("invalid artifact directory")?;
        let mut items: Vec<(Artifact, Bytes)> = Vec::new();
        let mut total = 0;
        for item in manifest.artifacts {
            if !token(&item.id, 32)
                || !filename(&item.file)
                || !text(&item.title, 120)
                || !text(&item.license, 500)
                || !text(&item.instructions, 1500)
                || !source_url(&item.source_url)
                || !(1..=MAX_FILE).contains(&item.size)
                || item.sha256.len() != 64
                || !item
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || items
                    .iter()
                    .any(|(old, _)| old.id == item.id || old.file == item.file)
            {
                return Err("invalid artifact manifest");
            }
            total += item.size;
            if total > MAX_TOTAL {
                return Err("artifact catalog too large");
            }
            let content = regular(&root.join(&item.file), item.size)?;
            if content.len() as u64 != item.size
                || hex::encode(Sha256::digest(&content)) != item.sha256
            {
                return Err("artifact integrity mismatch");
            }
            items.push((item, Bytes::from(content)));
        }
        Ok(Self {
            items,
            streams: Arc::new(Semaphore::new(4)),
        })
    }
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub(crate) async fn page(State(state): State<HttpState>) -> Response {
    let Some(web) = state.website.as_ref() else {
        return notice(StatusCode::NOT_FOUND, "Pobieranie jest wyłączone.");
    };
    let Some(catalog) = web.downloads.as_ref() else {
        return notice(StatusCode::NOT_FOUND, "Pobieranie jest wyłączone.");
    };
    let mut cards = String::new();
    for (item, _) in &catalog.items {
        let kind = match item.kind {
            Kind::Client => "Plik klienta",
            Kind::Installer => "Instalator",
        };
        cards.push_str(&format!("<section class=\"artifact\"><p class=\"badge\">{kind} · {} B</p><h2>{}</h2><p>{}</p><a class=\"button\" href=\"/downloads/{}\" download=\"{}\">Pobierz {}</a><details><summary>Pochodzenie i suma kontrolna</summary><p><a href=\"{}\" rel=\"noreferrer noopener\">Źródło pliku</a></p><p>{}</p><p>SHA-256: <code>{}</code></p></details></section>",item.size,escape(&item.title),escape(&item.instructions),item.id,item.file,escape(&item.title),escape(&item.source_url),escape(&item.license),item.sha256));
    }
    let links = format!(
        "{}{}{}{}",
        if web.statistics.is_some() {
            "<a href=\"/\">Strona główna</a> "
        } else {
            ""
        },
        if web.service.is_some() {
            "<a href=\"/register\">Załóż konto</a>"
        } else {
            ""
        },
        if web.patcher.is_some() {
            " <a href=\"/patcher\">Przygotuj własny plik GG</a>"
        } else {
            ""
        },
        if web.passwords.is_some() {
            " <a href=\"/password\">Zmień hasło</a>"
        } else {
            ""
        }
    );
    Html(
        include_str!("downloads.html")
            .replace("{{cards}}", &cards)
            .replace("{{links}}", &links),
    )
    .into_response()
}
#[derive(Serialize)]
struct PublicManifest<'a> {
    schema: u8,
    artifacts: Vec<&'a Artifact>,
}
pub(crate) async fn manifest(State(state): State<HttpState>) -> Response {
    let Some(catalog) = state.website.as_ref().and_then(|w| w.downloads.as_ref()) else {
        return notice(StatusCode::NOT_FOUND, "Pobieranie jest wyłączone.");
    };
    Json(PublicManifest {
        schema: 1,
        artifacts: catalog.items.iter().map(|(item, _)| item).collect(),
    })
    .into_response()
}
pub(crate) async fn file(
    State(state): State<HttpState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(catalog) = state.website.as_ref().and_then(|w| w.downloads.as_ref()) else {
        return notice(StatusCode::NOT_FOUND, "Pobieranie jest wyłączone.");
    };
    let Some((item, content)) = catalog.items.iter().find(|(item, _)| item.id == id) else {
        return notice(StatusCode::NOT_FOUND, "Nie ma takiego pliku.");
    };
    if headers.contains_key(header::RANGE) {
        return (
            StatusCode::RANGE_NOT_SATISFIABLE,
            [(header::CONTENT_RANGE, format!("bytes */{}", item.size))],
        )
            .into_response();
    }
    let Ok(permit) = catalog.streams.clone().try_acquire_owned() else {
        return notice(
            StatusCode::TOO_MANY_REQUESTS,
            "Wszystkie pobrania są zajęte. Spróbuj ponownie.",
        );
    };
    // The response stream retains the permit; disconnect drops it. Bytes slices share one
    // verified snapshot, so concurrent slow clients cannot duplicate the full artifact in RAM.
    let stream = futures_util::stream::unfold(
        (content.clone(), permit),
        |(mut content, permit)| async move {
            if content.is_empty() {
                None
            } else {
                let chunk = content.split_to(content.len().min(64 * 1024));
                Some((Ok::<_, std::convert::Infallible>(chunk), (content, permit)))
            }
        },
    );
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", item.file),
            ),
            (header::CONTENT_LENGTH, item.size.to_string()),
            (header::ACCEPT_RANGES, "none".to_owned()),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_text_and_paths_are_bounded_and_html_is_escaped() {
        for name in ["../a.exe", "x/y", "x\\y", "a:b", ".", "..", "a\r\nb"] {
            assert!(!filename(name));
        }
        assert!(filename("gg-patched_61.exe"));
        assert!(!token("a.b", 32));
        assert!(!source_url("javascript:alert(1)"));
        assert!(!source_url("https://x@y/"));
        assert!(!source_url("https://x\r\n/"));
        assert_eq!(escape("<a x=\"&\">'"), "&lt;a x=&quot;&amp;&quot;&gt;&#39;");
        assert!(!text("x\0", 10));
    }
}
