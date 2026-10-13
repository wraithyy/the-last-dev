//! Accept loop with connection-level defences that `axum::serve` does not offer: a request-head
//! timeout (slowloris), a global TCP cap and a per-IP cap. Both caps are held by a `Lease` that
//! WebSocket connections keep after the upgrade, so upgraded sockets still count.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::header::{
    CONTENT_SECURITY_POLICY, HeaderValue, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::http::{HeaderMap, Request};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};
use tokio::task::JoinSet;
use tower::ServiceExt;

use crate::AppState;
use crate::abuse::{ConnGuard, is_lan};

/// Resources tied to one TCP connection; released when the HTTP task and any upgraded
/// WebSocket task have both finished.
pub struct Lease {
    _tcp: OwnedSemaphorePermit,
    _ip: Option<ConnGuard>,
}

// 'unsafe-inline' scripts/styles: web/index.html has inline <script> blocks and a <style>.
// 'wasm-unsafe-eval' lets the wasm client compile. Upgrade path: serve a sha256 per inline
// script computed from index.html at startup and drop 'unsafe-inline' from script-src.
const CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; \
connect-src 'self' ws: wss:; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; \
worker-src 'self'";

pub fn add_security_headers(h: &mut HeaderMap) {
    h.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
}

pub async fn serve(
    listener: TcpListener,
    app: Router,
    st: Arc<AppState>,
    mut stopping: watch::Receiver<bool>,
) {
    // Twice max_conns: HTTP requests and sockets still sending headers also need slots.
    let tcp_slots = Arc::new(Semaphore::new(st.cfg.max_conns.saturating_mul(2).max(1)));
    let mut tasks = JoinSet::new();
    loop {
        // Acquire before accepting so overload backs up into the kernel queue instead of
        // being accepted and then dropped.
        let tcp = tokio::select! {
            p = tcp_slots.clone().acquire_owned() => match p {
                Ok(p) => p,
                Err(_) => break,
            },
            _ = async { let _ = stopping.wait_for(|v| *v).await; } => break,
            Some(_) = tasks.join_next(), if !tasks.is_empty() => continue,
        };
        let (stream, peer) = tokio::select! {
            r = listener.accept() => match r {
                Ok(x) => x,
                Err(e) => {
                    // Typically EMFILE; back off instead of spinning.
                    tracing::warn!(error = %e, "accept failed");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            },
            _ = async { let _ = stopping.wait_for(|v| *v).await; } => break,
        };
        // Behind a trusted proxy every client shares the proxy's address, so only the
        // global caps can apply there. LAN peers are exempt on a LAN server.
        let exempt =
            st.cfg.trusted_proxies.contains(&peer.ip()) || (st.cfg.lan_exempt && is_lan(peer.ip()));
        let ip = if exempt {
            None
        } else {
            match st.ip_conns.acquire(peer.ip()) {
                Some(g) => Some(g),
                None => {
                    tracing::warn!(peer = %peer.ip(), "per-IP connection cap reached");
                    continue;
                }
            }
        };
        let lease = Arc::new(Lease { _tcp: tcp, _ip: ip });
        tasks.spawn(handle(
            stream,
            peer,
            app.clone(),
            lease,
            st.cfg.header_read_timeout,
            stopping.clone(),
        ));
    }
    while tasks.join_next().await.is_some() {}
}

async fn handle(
    stream: tokio::net::TcpStream,
    peer: SocketAddr,
    app: Router,
    lease: Arc<Lease>,
    header_timeout: Duration,
    mut stopping: watch::Receiver<bool>,
) {
    // Latency matters more than packing for a realtime game; failure is harmless.
    let _ = stream.set_nodelay(true);
    let svc = service_fn(move |mut req: Request<Incoming>| {
        let app = app.clone();
        req.extensions_mut().insert(ConnectInfo(peer));
        req.extensions_mut().insert(lease.clone());
        async move {
            let mut resp = app
                .oneshot(req.map(Body::new))
                .await
                .unwrap_or_else(|e: Infallible| match e {});
            add_security_headers(resp.headers_mut());
            Ok::<_, Infallible>(resp)
        }
    });
    let mut builder = http1::Builder::new();
    // hyper requires a timer for header_read_timeout to take effect.
    builder.timer(TokioTimer::new());
    builder.header_read_timeout(header_timeout);
    let conn = builder
        .serve_connection(TokioIo::new(stream), svc)
        .with_upgrades();
    tokio::pin!(conn);
    tokio::select! {
        r = conn.as_mut() => {
            if let Err(e) = r {
                tracing::debug!(%peer, error = %e, "connection ended with error");
            }
        }
        _ = async { let _ = stopping.wait_for(|v| *v).await; } => {
            conn.as_mut().graceful_shutdown();
            let _ = conn.await;
        }
    }
}
