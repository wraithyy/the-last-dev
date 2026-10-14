//! `/ws` endpoint: origin-checked, binary-only, size-limited, rate-limited, idle-timed.
//! Runs the `net_proto` handshake, then decodes, validates and rate limits every message
//! before it reaches the room.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Extension, State};
use axum::http::header::{HOST, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use net_proto::{ClientMsg, ErrorKind, PROTOCOL_VERSION, ProtoError, ServerMsg, decode, encode};
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::{Instant, timeout, timeout_at};

use crate::AppState;
use crate::abuse::{ConnGuard, client_ip, is_lan};
use crate::limiter::TokenBucket;
use crate::rooms::{CANNOT_JOIN, JoinError, Joined, SendResult, code_tag};
use crate::serve::Lease;

const SEND_TIMEOUT: Duration = Duration::from_secs(5);

/// The close to send after a failed read, or None when the peer is simply gone. A client that
/// exits drops its TCP connection, often without a close handshake; that is not an attack and
/// must not be reported as an oversized or malformed message.
pub fn read_error_close(e: &tungstenite::Error) -> Option<(u16, &'static str)> {
    use tungstenite::error::ProtocolError;
    match e {
        tungstenite::Error::Capacity(_) => Some((1009, "message too large")),
        tungstenite::Error::Protocol(ProtocolError::ResetWithoutClosingHandshake) => None,
        tungstenite::Error::Protocol(_) | tungstenite::Error::Utf8(_) => {
            Some((1002, "malformed frame"))
        }
        _ => None,
    }
}

fn classify(e: axum::Error) -> Option<(u16, &'static str)> {
    let inner = e.into_inner();
    let close = inner
        .downcast_ref::<tungstenite::Error>()
        .and_then(read_error_close);
    if close.is_none() {
        tracing::debug!(error = %inner, "websocket read ended");
    }
    close
}

/// Browsers send `Origin` on WebSocket upgrades and let any page open a socket to any host, so
/// without this check a malicious site could drive a visitor's connection to a LAN/localhost
/// server (cross-site WebSocket hijacking). Native clients send no Origin.
pub fn origin_allowed(headers: &HeaderMap, allowed: &[String]) -> bool {
    let Some(origin) = headers.get(ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let origin = origin.trim_end_matches('/').to_ascii_lowercase();
    if allowed.contains(&origin) {
        return true;
    }
    let origin_host = origin.split_once("://").map(|(_, h)| h);
    let host = headers
        .get(HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::to_ascii_lowercase);
    matches!((origin_host, host), (Some(o), Some(h)) if o == h)
}

/// Per-connection counter of invalid messages. The protocol layer calls `bump` for each message
/// it rejects; the connection closes once the budget is spent.
pub struct Strikes {
    left: u32,
}

impl Strikes {
    pub fn new(limit: u32) -> Self {
        Self { left: limit }
    }

    /// Records one strike; true means the connection must be closed. A limit of 0 disables.
    pub fn bump(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.left -= 1;
        self.left == 0
    }
}

/// `/ws`: the protocol handshake (`Hello`, then `CreateRoom` or `JoinRoom`) runs over the socket.
pub async fn ws_handler(
    State(st): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Extension(lease): Extension<Arc<Lease>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !origin_allowed(&headers, &st.cfg.allowed_origins) {
        tracing::warn!(peer = %peer.ip(), "rejected websocket upgrade: origin not allowed");
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    let ip = client_ip(
        peer.ip(),
        &headers,
        &st.cfg.trusted_proxies,
        st.cfg.trust_forwarded_for,
    );
    // Behind a proxy the TCP-level lease skipped the per-IP cap (every client shares the
    // proxy's address); apply it to the resolved client IP instead.
    let ip_guard = if !lease.counts_ip() && limited(&st, ip) {
        match st.ip_conns.acquire(ip) {
            Some(g) => Some(g),
            None => {
                tracing::warn!(%ip, "per-IP connection cap reached");
                return (StatusCode::TOO_MANY_REQUESTS, "too many connections").into_response();
            }
        }
    } else {
        None
    };
    // Held for the whole connection lifetime; this is the global connection cap.
    let Ok(permit) = st.conns.clone().try_acquire_owned() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "server is at its connection limit",
        )
            .into_response();
    };
    let max = st.cfg.max_message_bytes;
    ws.max_message_size(max)
        .max_frame_size(max)
        .on_upgrade(move |socket| connection(socket, st, ip, (permit, ip_guard), lease))
}

async fn close(socket: &mut WebSocket, code: u16, reason: &'static str) {
    let frame = CloseFrame {
        code,
        reason: reason.into(),
    };
    // Best effort: the peer may already be gone.
    let _ = timeout(SEND_TIMEOUT, socket.send(Message::Close(Some(frame)))).await;
}

async fn send_msg(socket: &mut WebSocket, msg: &ServerMsg) -> bool {
    let Ok(bytes) = encode(msg) else {
        return false;
    };
    matches!(
        timeout(SEND_TIMEOUT, socket.send(Message::Binary(bytes.into()))).await,
        Ok(Ok(()))
    )
}

/// Sends the protocol error, then closes.
async fn reject(socket: &mut WebSocket, kind: ErrorKind, code: u16, reason: &'static str) {
    let err = ServerMsg::Error {
        kind,
        text: kind.message().into(),
    };
    send_msg(socket, &err).await;
    close(socket, code, reason).await;
}

/// Whether per-IP limits apply to this client.
fn limited(st: &AppState, ip: IpAddr) -> bool {
    // lan_exempt is forced off with trust_forwarded_for at parse time; checked again so a
    // hand-built Config cannot exempt clients whose address a header chose.
    !(st.cfg.lan_exempt && !st.cfg.trust_forwarded_for && is_lan(ip))
}

enum Recv {
    Msg(ClientMsg),
    Bad(ProtoError),
    Timeout,
    /// Close code and reason to send; None when the peer is already gone.
    End(Option<(u16, &'static str)>),
}

/// Next binary frame before `until`, decoded. Control frames are skipped.
async fn recv(socket: &mut WebSocket, until: Instant, bucket: &mut TokenBucket) -> Recv {
    loop {
        let msg = match timeout_at(until, socket.recv()).await {
            Err(_) => return Recv::Timeout,
            Ok(None) => return Recv::End(None),
            Ok(Some(Err(e))) => return Recv::End(classify(e)),
            Ok(Some(Ok(m))) => m,
        };
        if !bucket.try_take() {
            return Recv::End(Some((1008, "rate limit exceeded")));
        }
        match msg {
            Message::Binary(b) => {
                return match decode::<ClientMsg>(&b) {
                    Ok(m) => Recv::Msg(m),
                    Err(e) => Recv::Bad(e),
                };
            }
            Message::Text(_) => return Recv::End(Some((1003, "binary frames only"))),
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Close(_) => return Recv::End(None),
        }
    }
}

async fn connection(
    mut socket: WebSocket,
    st: Arc<AppState>,
    ip: IpAddr,
    _held: (OwnedSemaphorePermit, Option<ConnGuard>),
    _lease: Arc<Lease>,
) {
    let limited = limited(&st, ip);
    let mut bucket = TokenBucket::new(st.cfg.rate_per_sec, st.cfg.rate_burst);
    let hs = st.cfg.handshake_timeout;

    let (name, class) = match recv(&mut socket, Instant::now() + hs, &mut bucket).await {
        Recv::Msg(ClientMsg::Hello {
            version,
            name,
            class,
        }) if version == PROTOCOL_VERSION => {
            if !st.registry.knows_class(&class) {
                return reject(&mut socket, ErrorKind::UnknownClass, 1008, "unknown class").await;
            }
            (name, class)
        }
        Recv::Msg(ClientMsg::Hello { .. }) | Recv::Bad(ProtoError::BadVersion(_)) => {
            return reject(
                &mut socket,
                ErrorKind::BadVersion,
                1008,
                "protocol version mismatch",
            )
            .await;
        }
        Recv::Timeout => {
            return reject(
                &mut socket,
                ErrorKind::HelloRequired,
                1008,
                "handshake timeout",
            )
            .await;
        }
        Recv::End(Some((code, reason))) => return close(&mut socket, code, reason).await,
        Recv::End(None) => return,
        Recv::Msg(_) | Recv::Bad(_) => {
            return reject(
                &mut socket,
                ErrorKind::HelloRequired,
                1008,
                "hello required",
            )
            .await;
        }
    };

    let joined = match recv(&mut socket, Instant::now() + hs, &mut bucket).await {
        Recv::Msg(ClientMsg::CreateRoom) => {
            if limited && !st.room_creates.check_and_hit(ip) {
                tracing::warn!(%ip, "room creation rate limit hit");
                return reject(
                    &mut socket,
                    ErrorKind::RateLimited,
                    1008,
                    "too many rooms created",
                )
                .await;
            }
            match st.registry.create_and_join(&name, &class) {
                Ok(j) => {
                    if !send_msg(
                        &mut socket,
                        &ServerMsg::RoomCreated {
                            code: j.code.clone(),
                        },
                    )
                    .await
                    {
                        st.registry.leave(&j).await;
                        return;
                    }
                    Ok(j)
                }
                Err(e) => Err(e),
            }
        }
        // `decode` already normalised the code; a malformed one fails as BadRoomCode.
        Recv::Msg(ClientMsg::JoinRoom { code }) => {
            join(&st, ip, limited, Some(&code), (&name, &class))
        }
        Recv::Bad(ProtoError::BadRoomCode) => join(&st, ip, limited, None, (&name, &class)),
        Recv::Timeout => {
            return reject(
                &mut socket,
                ErrorKind::HelloRequired,
                1008,
                "handshake timeout",
            )
            .await;
        }
        Recv::End(Some((code, reason))) => return close(&mut socket, code, reason).await,
        Recv::End(None) => return,
        Recv::Msg(_) | Recv::Bad(_) => {
            return reject(
                &mut socket,
                ErrorKind::BadMessage,
                1008,
                "expected create or join",
            )
            .await;
        }
    };
    let mut joined = match joined {
        Ok(j) => j,
        Err(JoinError::TooManyRooms) => {
            return reject(
                &mut socket,
                ErrorKind::ServerFull,
                1008,
                JoinError::TooManyRooms.reason(),
            )
            .await;
        }
        Err(_) => return reject(&mut socket, ErrorKind::CannotJoin, 1008, CANNOT_JOIN).await,
    };
    let result = pump(&mut socket, &st, &mut joined, bucket).await;
    st.registry.leave(&joined).await;
    if let Some((code, reason)) = result {
        tracing::info!(room = %code_tag(&joined.code), player = joined.player, reason, "closing connection");
        close(&mut socket, code, reason).await;
    }
}

/// `code` is None for a code that did not parse. Every failure counts towards the lockout.
fn join(
    st: &AppState,
    ip: IpAddr,
    limited: bool,
    code: Option<&str>,
    (name, class): (&str, &str),
) -> Result<Joined, JoinError> {
    if limited && !st.join_fails.allowed(ip) {
        tracing::warn!(%ip, "join attempt while locked out");
        return Err(JoinError::Unavailable);
    }
    let r = code.map_or(Err(JoinError::NoSuchRoom), |c| {
        st.registry.join(c, name, class)
    });
    if let Err(e) = &r {
        if limited {
            st.join_fails.hit(ip);
        }
        tracing::info!(%ip, room = %code.map(code_tag).unwrap_or_default(), error = ?e, "join failed");
    }
    r
}

/// Per-message-type budgets on top of the connection-wide bucket.
struct TypeLimits {
    cmd: TokenBucket,
    mv: TokenBucket,
    emote: TokenBucket,
}

impl TypeLimits {
    fn new() -> Self {
        Self {
            cmd: TokenBucket::new(20.0, 20.0),
            mv: TokenBucket::new(30.0, 45.0),
            emote: TokenBucket::new(1.0, 3.0),
        }
    }
}

/// Returns the close code and reason to send, or None if the peer already closed.
async fn pump(
    socket: &mut WebSocket,
    st: &AppState,
    joined: &mut Joined,
    mut bucket: TokenBucket,
) -> Option<(u16, &'static str)> {
    let idle = st.cfg.idle_timeout;
    let mut strikes = Strikes::new(st.cfg.invalid_message_limit);
    let mut limits = TypeLimits::new();
    let mut deadline = Instant::now() + idle;
    loop {
        tokio::select! {
            out = joined.outbound.recv() => match out {
                Some(bytes) => {
                    if !matches!(timeout(SEND_TIMEOUT, socket.send(Message::Binary(bytes))).await, Ok(Ok(()))) {
                        return None;
                    }
                }
                // Room ended, server is shutting down, or this client fell behind.
                None => return Some((1013, "room closed or client too slow")),
            },
            incoming = timeout_at(deadline, socket.recv()) => match incoming {
                Err(_) => return Some((1001, "idle timeout")),
                Ok(None) => return None,
                // Oversized messages, malformed frames and dropped peers surface here.
                Ok(Some(Err(e))) => return classify(e),
                Ok(Some(Ok(msg))) => {
                    if !bucket.try_take() {
                        return Some((1008, "rate limit exceeded"));
                    }
                    let b = match msg {
                        // Only real traffic keeps a connection alive; control frames are free
                        // to send and would otherwise pin a slot forever.
                        Message::Binary(b) => {
                            deadline = Instant::now() + idle;
                            b
                        }
                        Message::Text(_) => return Some((1003, "binary frames only")),
                        Message::Ping(_) | Message::Pong(_) => continue,
                        Message::Close(_) => return None,
                    };
                    // None: invalid here or over its type budget, which costs a strike.
                    let forward = match decode::<ClientMsg>(&b) {
                        Ok(ClientMsg::Ping { t }) => {
                            // Answered here so the round trip does not include the room's tick.
                            if !send_msg(socket, &ServerMsg::Pong { t }).await {
                                return None;
                            }
                            continue;
                        }
                        Ok(ClientMsg::LeaveRoom) => return Some((1000, "left room")),
                        Ok(m @ ClientMsg::Cmd(_)) => limits.cmd.try_take().then_some(m),
                        Ok(m @ ClientMsg::Move { .. }) => limits.mv.try_take().then_some(m),
                        // Emote spam is merely dropped: an impatient player is not an attacker.
                        Ok(m @ ClientMsg::Emote { .. }) if limits.emote.try_take() => Some(m),
                        Ok(ClientMsg::Emote { .. }) => continue,
                        Ok(ClientMsg::Hello { .. } | ClientMsg::CreateRoom | ClientMsg::JoinRoom { .. })
                        | Err(_) => None,
                    };
                    let Some(m) = forward else {
                        if strikes.bump() {
                            return Some((1008, "too many invalid messages"));
                        }
                        continue;
                    };
                    match joined.send(m) {
                        SendResult::Queued => {}
                        SendResult::Full => return Some((1008, "sending too fast")),
                        SendResult::Closed => return Some((1013, "room closed or client too slow")),
                    }
                }
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn hdrs(origin: Option<&str>, host: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(HOST, host.parse().unwrap());
        if let Some(o) = origin {
            h.insert(ORIGIN, o.parse().unwrap());
        }
        h
    }

    #[test]
    fn origin_rules() {
        let allow = vec!["https://wraithyy.github.io".to_string()];
        assert!(origin_allowed(&hdrs(None, "h:1"), &[]));
        assert!(origin_allowed(&hdrs(Some("http://h:1"), "h:1"), &[]));
        assert!(origin_allowed(&hdrs(Some("HTTP://H:1/"), "h:1"), &[]));
        assert!(!origin_allowed(
            &hdrs(Some("http://evil.example"), "h:1"),
            &[]
        ));
        assert!(!origin_allowed(&hdrs(Some("http://h:2"), "h:1"), &[]));
        assert!(!origin_allowed(&hdrs(Some("null"), "h:1"), &[]));
        assert!(!origin_allowed(&hdrs(Some("http://h:1/x"), "h:1"), &[]));
        assert!(origin_allowed(
            &hdrs(Some("https://wraithyy.github.io"), "h:1"),
            &allow
        ));
        assert!(!origin_allowed(
            &hdrs(Some("https://wraithyy.github.io.evil.example"), "h:1"),
            &allow
        ));
    }

    #[test]
    fn dropped_peers_are_not_malformed_messages() {
        use tungstenite::error::{CapacityError, ProtocolError};
        let reset = tungstenite::Error::Protocol(ProtocolError::ResetWithoutClosingHandshake);
        assert_eq!(read_error_close(&reset), None);
        assert_eq!(
            read_error_close(&tungstenite::Error::ConnectionClosed),
            None
        );
        let io = std::io::Error::from(std::io::ErrorKind::ConnectionReset);
        assert_eq!(read_error_close(&tungstenite::Error::Io(io)), None);
        let big = tungstenite::Error::Capacity(CapacityError::MessageTooLong {
            size: 9,
            max_size: 1,
        });
        assert_eq!(read_error_close(&big).map(|c| c.0), Some(1009));
        let bad = tungstenite::Error::Protocol(ProtocolError::UnmaskedFrameFromClient);
        assert_eq!(read_error_close(&bad).map(|c| c.0), Some(1002));
    }

    #[test]
    fn strikes_close_after_limit() {
        let mut s = Strikes::new(3);
        assert!(!s.bump() && !s.bump());
        assert!(s.bump());
        let mut off = Strikes::new(0);
        assert!(!off.bump());
    }
}
