//! Static file serving hardened around `ServeDir`: no dotfiles, no symlinks out of the web dir,
//! and correct types for the wasm client's files.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use tower::ServiceExt;
use tower_http::services::ServeDir;

pub struct StaticFiles {
    root: PathBuf,
    serve: ServeDir,
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The request path relative to the root, or None if any segment is hidden or suspicious.
/// Decoded before checking so `%2e` cannot smuggle a dot past the filter.
fn safe_rel(path: &str) -> Option<PathBuf> {
    let decoded = percent_decode(path)?;
    let mut rel = PathBuf::new();
    for seg in decoded.split('/').filter(|s| !s.is_empty()) {
        if seg.starts_with('.') || seg.contains(['\\', '\0']) {
            return None;
        }
        rel.push(seg);
    }
    Some(rel)
}

fn content_type_override(rel: &Path) -> Option<&'static str> {
    match rel.extension()?.to_str()? {
        "wasm" => Some("application/wasm"),
        "webmanifest" => Some("application/manifest+json"),
        _ => None,
    }
}

impl StaticFiles {
    pub fn new(dir: &Path) -> std::io::Result<Self> {
        let root = dir.canonicalize()?;
        Ok(Self {
            serve: ServeDir::new(&root),
            root,
        })
    }

    /// True when the path resolves (through symlinks) outside the root.
    async fn escapes(&self, p: PathBuf) -> bool {
        match tokio::fs::canonicalize(&p).await {
            Ok(real) => !real.starts_with(&self.root),
            // Nonexistent: ServeDir will 404 it.
            Err(_) => false,
        }
    }

    pub async fn serve(&self, req: Request<Body>) -> Response {
        let not_found = || StatusCode::NOT_FOUND.into_response();
        let Some(rel) = safe_rel(req.uri().path()) else {
            return not_found();
        };
        let full = self.root.join(&rel);
        // The directory index is resolved by ServeDir, so check it explicitly too.
        // A symlink swapped in between this check and the open is out of scope: the web dir
        // is operator-owned.
        if self.escapes(full.clone()).await || self.escapes(full.join("index.html")).await {
            return not_found();
        }
        let mut resp = match self.serve.clone().oneshot(req).await {
            Ok(r) => r.into_response(),
            Err(e) => match e {}, // Infallible
        };
        if resp.status().is_success()
            && let Some(ct) = content_type_override(&rel)
        {
            resp.headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_static(ct));
        }
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_paths() {
        assert_eq!(safe_rel("/pkg/a.js"), Some(PathBuf::from("pkg/a.js")));
        assert_eq!(safe_rel("/"), Some(PathBuf::new()));
        for bad in [
            "/.env",
            "/a/.git/config",
            "/%2e%2e/x",
            "/%2Eenv",
            "/a%5Cb",
            "/%00",
            "/%zz",
            "/.well-known/x",
        ] {
            assert_eq!(safe_rel(bad), None, "{bad}");
        }
    }
}
