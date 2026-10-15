//! Self-hostable co-op server for The Last Dev.
//!
//! Each room runs an authoritative `combat_core::Sim` (`room_state`) and speaks `net_proto`.

pub mod abuse;
pub mod config;
pub mod limiter;
pub mod room_state;
pub mod rooms;
pub mod serve;
pub mod static_files;
pub mod world;
pub mod ws;

use std::future::Future;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use abuse::{ConnTracker, RateLimiter, client_ip};
use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::HeaderMap;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use config::Config;
use rooms::{RoomRegistry, random_room_code};
use tokio::net::TcpListener;
use tokio::sync::{Semaphore, watch};

pub struct AppState {
    pub cfg: Config,
    pub registry: RoomRegistry,
    pub conns: Arc<Semaphore>,
    pub ip_conns: Arc<ConnTracker>,
    pub join_fails: RateLimiter,
    pub room_creates: RateLimiter,
    /// `ListRooms` and `JoinPublic` per IP.
    pub room_lists: RateLimiter,
    pub statics: Option<static_files::StaticFiles>,
}

const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

/// Counts are only for the operator: anyone else gets "ok", which is all a load balancer needs.
async fn healthz(
    State(st): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let ip = client_ip(
        peer.ip(),
        &headers,
        &st.cfg.trusted_proxies,
        st.cfg.trust_forwarded_for,
    );
    // A loopback peer that forwards for someone (an untrusted local proxy) is not the operator.
    let forwarded = !st.cfg.trust_forwarded_for
        && !st.cfg.trusted_proxies.contains(&peer.ip())
        && headers.contains_key("x-forwarded-for");
    if !(st.cfg.healthz_details || (ip.is_loopback() && !forwarded)) {
        return ([(CONTENT_TYPE, "text/plain")], "ok").into_response();
    }
    let (rooms, players) = st.registry.counts();
    let conns = st
        .cfg
        .max_conns
        .saturating_sub(st.conns.available_permits());
    (
        [(CONTENT_TYPE, "application/json")],
        format!(
            r#"{{"status":"ok","rooms":{rooms},"players":{players},"connections":{conns},"tick_p99_us":{}}}"#,
            st.registry.tick_p99_us()
        ),
    )
        .into_response()
}

async fn static_handler(State(st): State<Arc<AppState>>, req: Request) -> Response {
    match &st.statics {
        Some(s) => s.serve(req).await,
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

/// Serves until `shutdown` resolves, then closes all rooms and drains connections.
pub async fn run(
    listener: TcpListener,
    cfg: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let world = world::World::load_from(cfg.content.as_deref()).map_err(std::io::Error::other)?;
    let registry = RoomRegistry::new(&cfg, random_room_code, world);
    let statics = match static_files::StaticFiles::new(&cfg.web_dir) {
        Ok(s) => Some(s),
        Err(_) => {
            tracing::warn!(dir = %cfg.web_dir.display(), "web dir missing; serving /ws and /healthz only");
            None
        }
    };
    let st = Arc::new(AppState {
        conns: Arc::new(Semaphore::new(cfg.max_conns)),
        ip_conns: ConnTracker::new(cfg.per_ip_conns),
        join_fails: RateLimiter::new(
            cfg.per_ip_failed_joins,
            Duration::from_secs(60),
            cfg.failed_join_lockout,
        ),
        room_creates: RateLimiter::new(
            cfg.per_ip_rooms_per_min,
            Duration::from_secs(60),
            Duration::ZERO,
        ),
        room_lists: RateLimiter::new(
            cfg.per_ip_lists_per_min,
            Duration::from_secs(60),
            Duration::ZERO,
        ),
        statics,
        registry: registry.clone(),
        cfg,
    });

    let app = Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/healthz", get(healthz))
        .fallback(static_handler)
        .with_state(st.clone());

    let gauge = {
        let st = st.clone();
        tokio::spawn(async move {
            let mut t = tokio::time::interval(Duration::from_secs(60));
            loop {
                t.tick().await;
                let (rooms, players) = st.registry.counts();
                st.join_fails.prune();
                st.room_creates.prune();
                st.room_lists.prune();
                tracing::info!(rooms, players, "gauge");
            }
        })
    };

    let (stopping_tx, stopping_rx) = watch::channel(false);
    let watcher = tokio::spawn(async move {
        shutdown.await;
        tracing::info!("shutting down");
        registry.shutdown();
        stopping_tx.send_replace(true);
    });
    let mut grace_rx = stopping_rx.clone();
    tokio::select! {
        () = serve::serve(listener, app, st.clone(), stopping_rx) => {}
        // Upgraded sockets that refuse to close must not block exit forever.
        _ = async {
            let _ = grace_rx.wait_for(|v| *v).await;
            tokio::time::sleep(SHUTDOWN_GRACE).await;
        } => {
            tracing::warn!("grace period over, exiting with open connections");
        }
    }
    watcher.abort();
    gauge.abort();
    Ok(())
}

/// Outbound-interface IPv4 address. Connecting a UDP socket sends no packets; it only
/// makes the OS pick a route.
pub fn lan_ipv4() -> Option<IpAddr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(ip)
}
