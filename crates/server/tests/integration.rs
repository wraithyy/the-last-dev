#![allow(clippy::unwrap_used, clippy::expect_used)] // test code

use std::net::SocketAddr;
use std::time::Duration;

use combat_core::{Aabb, Command, EntityId, Event, Sim, SpellId, Vec3, line_of_sight};
use futures_util::{SinkExt, StreamExt};
use net_proto::{
    ClientMsg, Emote, ErrorKind, PROTOCOL_VERSION, ServerMsg, UnitSnap, decode, encode,
};
use server::config::Config;
use server::world::World;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

struct Running {
    addr: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}

async fn start(cfg: Config) -> Running {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server::run(listener, cfg, async {
        let _ = rx.await;
    }));
    Running {
        addr,
        stop: Some(stop),
        task,
    }
}

impl Running {
    async fn shutdown(mut self) {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(5), self.task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

fn cfg() -> Config {
    Config {
        web_dir: "/nonexistent-web-dir".into(),
        ..Config::default()
    }
}

async fn http_get(addr: SocketAddr, path: &str) -> (String, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut buf = String::new();
    s.read_to_string(&mut buf).await.unwrap();
    let (head, body) = buf.split_once("\r\n\r\n").unwrap();
    (head.lines().next().unwrap().to_string(), body.to_string())
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

async fn connect(addr: SocketAddr, query: &str) -> Ws {
    tokio_tungstenite::connect_async(format!("ws://{addr}/ws{query}"))
        .await
        .unwrap()
        .0
}

async fn connect_with(
    addr: SocketAddr,
    query: &str,
    headers: &[(&'static str, &str)],
) -> Result<Ws, tokio_tungstenite::tungstenite::Error> {
    let mut req = format!("ws://{addr}/ws{query}")
        .into_client_request()
        .unwrap();
    for (k, v) in headers {
        req.headers_mut().insert(*k, v.parse().unwrap());
    }
    tokio_tungstenite::connect_async(req).await.map(|r| r.0)
}

/// Like `expect_closed` but also returns the close reason.
async fn close_info(ws: &mut Ws) -> Option<(CloseCode, String)> {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Close(f))) => return f.map(|f| (f.code, f.reason.to_string())),
            Some(Ok(_)) => continue,
            Some(Err(_)) | None => return None,
        }
    }
}

/// Raw request with extra headers; returns (head, body).
async fn http_raw(addr: SocketAddr, path: &str, extra: &str) -> (String, String) {
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(
        format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n{extra}\r\n").as_bytes(),
    )
    .await
    .unwrap();
    let mut buf = String::new();
    s.read_to_string(&mut buf).await.unwrap();
    let (head, body) = buf.split_once("\r\n\r\n").unwrap();
    (head.to_string(), body.to_string())
}

fn strict() -> Config {
    Config {
        lan_exempt: false,
        ..cfg()
    }
}

fn enc(m: &ClientMsg) -> Message {
    Message::Binary(encode(m).unwrap().into())
}

fn hello_msg(name: &str) -> Message {
    enc(&ClientMsg::Hello {
        version: PROTOCOL_VERSION,
        name: name.into(),
        class: "fullstack".into(),
    })
}

/// Next protocol message; panics on close or after 5 s.
async fn next_msg(ws: &mut Ws) -> ServerMsg {
    loop {
        let m = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("timed out waiting for a message")
            .expect("stream ended")
            .expect("socket error");
        if let Message::Binary(b) = m {
            return decode(&b).unwrap();
        }
        assert!(!m.is_close(), "closed: {m:?}");
    }
}

async fn wait_for(ws: &mut Ws, pred: impl Fn(&ServerMsg) -> bool) -> ServerMsg {
    loop {
        let m = next_msg(ws).await;
        if pred(&m) {
            return m;
        }
    }
}

/// Messages already received, without waiting.
async fn drain_now(ws: &mut Ws) -> Vec<ServerMsg> {
    let mut out = Vec::new();
    while let Ok(Some(Ok(m))) = tokio::time::timeout(Duration::from_millis(1), ws.next()).await {
        if let Message::Binary(b) = m {
            out.push(decode(&b).unwrap());
        }
    }
    out
}

/// Connect, say hello and create a room: (socket, room code, own entity id).
async fn create_with(addr: SocketAddr, headers: &[(&'static str, &str)]) -> (Ws, String, EntityId) {
    let mut ws = connect_with(addr, "", headers).await.unwrap();
    ws.send(hello_msg("Dana")).await.unwrap();
    ws.send(enc(&ClientMsg::CreateRoom)).await.unwrap();
    let ServerMsg::RoomCreated { code } = next_msg(&mut ws).await else {
        panic!("expected RoomCreated")
    };
    let ServerMsg::Welcome { you, room, .. } = next_msg(&mut ws).await else {
        panic!("expected Welcome")
    };
    assert_eq!(room, code);
    (ws, code, you)
}

async fn create(addr: SocketAddr) -> (Ws, String, EntityId) {
    create_with(addr, &[]).await
}

/// Hello + JoinRoom without reading the answer.
async fn request_join(addr: SocketAddr, code: &str) -> Ws {
    let mut ws = connect(addr, "").await;
    ws.send(hello_msg("Bo")).await.unwrap();
    ws.send(enc(&ClientMsg::JoinRoom { code: code.into() }))
        .await
        .unwrap();
    ws
}

async fn join(addr: SocketAddr, code: &str) -> (Ws, EntityId) {
    let mut ws = request_join(addr, code).await;
    let ServerMsg::Welcome { you, .. } = next_msg(&mut ws).await else {
        panic!("expected Welcome")
    };
    (ws, you)
}

/// The refusal a failed join gets: the error frame, then the close.
async fn join_refusal(ws: &mut Ws) -> (ErrorKind, Option<(CloseCode, String)>) {
    let kind = match next_msg(ws).await {
        ServerMsg::Error { kind, .. } => kind,
        m => panic!("expected Error, got {m:?}"),
    };
    (kind, close_info(ws).await)
}

async fn snapshot(ws: &mut Ws) -> (Vec<UnitSnap>, Vec<EntityId>) {
    match wait_for(ws, |m| matches!(m, ServerMsg::Snapshot { .. })).await {
        ServerMsg::Snapshot { units, players, .. } => {
            (units, players.into_iter().map(|p| p.id).collect())
        }
        _ => unreachable!(),
    }
}

/// Reads until a Close frame or end of stream; returns the close code if there was one.
async fn expect_closed(ws: &mut Ws) -> Option<CloseCode> {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Close(f))) => return f.map(|f| f.code),
            Some(Ok(_)) => continue,
            Some(Err(_)) | None => return None,
        }
    }
}

#[tokio::test]
async fn rejects_text_frames() {
    let s = start(cfg()).await;
    let mut ws = connect(s.addr, "").await;
    ws.send(Message::Text("hello".into())).await.unwrap();
    assert_eq!(expect_closed(&mut ws).await, Some(CloseCode::Unsupported));
    s.shutdown().await;
}

#[tokio::test]
async fn rejects_oversize_messages() {
    let s = start(cfg()).await;
    let mut ws = connect(s.addr, "").await;
    ws.send(Message::Binary(vec![0u8; 64 * 1024 + 1].into()))
        .await
        .unwrap();
    // Either a Size close frame or an abrupt end; never an echo.
    let code = expect_closed(&mut ws).await;
    assert!(code.is_none() || code == Some(CloseCode::Size), "{code:?}");
    s.shutdown().await;
}

#[tokio::test]
async fn serves_static_files() {
    let dir = std::env::temp_dir().join(format!("tld-web-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("pkg")).unwrap();
    std::fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
    std::fs::write(dir.join("pkg/a.js"), "x=1").unwrap();
    let s = start(Config {
        web_dir: dir.clone(),
        ..cfg()
    })
    .await;
    let (status, body) = http_get(s.addr, "/").await;
    assert!(
        status.contains("200") && body == "<h1>hi</h1>",
        "{status} {body}"
    );
    let (status, body) = http_get(s.addr, "/pkg/a.js").await;
    assert!(status.contains("200") && body == "x=1");
    let (status, _) = http_get(s.addr, "/../Cargo.toml").await;
    assert!(status.contains("404") || status.contains("400"), "{status}");
    let (status, _) = http_get(s.addr, "/missing").await;
    assert!(status.contains("404"));
    s.shutdown().await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn cswsh_origin_check() {
    let s = start(Config {
        allowed_origins: vec!["https://wraithyy.github.io".into()],
        ..cfg()
    })
    .await;
    let addr = s.addr;
    let err = connect_with(addr, "", &[("origin", "https://evil.example")])
        .await
        .unwrap_err();
    assert!(err.to_string().contains("403"), "{err}");
    assert!(
        connect_with(addr, "", &[]).await.is_ok(),
        "no Origin: native client"
    );
    let same = format!("http://{addr}");
    assert!(connect_with(addr, "", &[("origin", &same)]).await.is_ok());
    assert!(
        connect_with(addr, "", &[("origin", "https://wraithyy.github.io")])
            .await
            .is_ok()
    );
    s.shutdown().await;
}

#[tokio::test]
async fn per_ip_connection_cap() {
    let s = start(Config {
        per_ip_conns: 2,
        ..strict()
    })
    .await;
    let _a = TcpStream::connect(s.addr).await.unwrap();
    let _b = TcpStream::connect(s.addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut c = TcpStream::connect(s.addr).await.unwrap();
    let mut buf = [0u8; 16];
    let n = tokio::time::timeout(Duration::from_secs(3), c.read(&mut buf))
        .await
        .unwrap();
    assert!(
        matches!(n, Ok(0) | Err(_)),
        "third connection must be closed: {n:?}"
    );
    // Exempt LAN peers are not capped by default.
    drop((_a, _b));
    s.shutdown().await;
    let s = start(Config {
        per_ip_conns: 1,
        ..cfg()
    })
    .await;
    let _x = TcpStream::connect(s.addr).await.unwrap();
    let (status, _) = http_get(s.addr, "/healthz").await;
    assert!(status.contains("200"));
    s.shutdown().await;
}

#[tokio::test]
async fn slow_headers_are_cut_off() {
    let s = start(Config {
        header_read_timeout: Duration::from_millis(300),
        ..cfg()
    })
    .await;
    let mut c = TcpStream::connect(s.addr).await.unwrap();
    c.write_all(b"GET / HT").await.unwrap();
    let mut buf = Vec::new();
    let r = tokio::time::timeout(Duration::from_secs(5), c.read_to_end(&mut buf)).await;
    assert!(r.is_ok(), "server kept a half-sent request open");
    s.shutdown().await;
}

#[tokio::test]
async fn healthz_hides_details_from_non_operators() {
    let s = start(cfg()).await;
    let (_, body) = http_raw(s.addr, "/healthz", "X-Forwarded-For: 203.0.113.5\r\n").await;
    assert_eq!(body, "ok");
    s.shutdown().await;

    let s = start(Config {
        trusted_proxies: vec!["127.0.0.1".parse().unwrap()],
        ..cfg()
    })
    .await;
    let (_, body) = http_raw(s.addr, "/healthz", "X-Forwarded-For: 203.0.113.5\r\n").await;
    assert_eq!(body, "ok");
    let (_, body) = http_raw(s.addr, "/healthz", "X-Forwarded-For: 127.0.0.1\r\n").await;
    assert!(body.contains("rooms"), "{body}");
    s.shutdown().await;

    let s = start(Config {
        healthz_details: true,
        ..cfg()
    })
    .await;
    let (_, body) = http_raw(s.addr, "/healthz", "X-Forwarded-For: 203.0.113.5\r\n").await;
    assert!(body.contains("rooms"), "{body}");
    s.shutdown().await;
}

#[tokio::test]
async fn static_hardening_and_headers() {
    let base = std::env::temp_dir().join(format!("tld-hard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("web");
    std::fs::create_dir_all(dir.join("pkg")).unwrap();
    std::fs::write(dir.join("index.html"), "<h1>hi</h1>").unwrap();
    std::fs::write(dir.join(".env"), "SECRET=1").unwrap();
    std::fs::write(dir.join("pkg/a.wasm"), [0, 97, 115, 109]).unwrap();
    std::fs::write(dir.join("manifest.webmanifest"), "{}").unwrap();
    std::fs::write(base.join("outside.txt"), "outside").unwrap();
    std::os::unix::fs::symlink(base.join("outside.txt"), dir.join("link.txt")).unwrap();
    std::os::unix::fs::symlink(&base, dir.join("up")).unwrap();
    std::fs::write(dir.join("pkg/ok.txt"), "ok").unwrap();
    std::os::unix::fs::symlink(dir.join("pkg/ok.txt"), dir.join("inside.txt")).unwrap();
    let s = start(Config {
        web_dir: dir.clone(),
        ..cfg()
    })
    .await;
    for p in ["/.env", "/%2eenv", "/link.txt", "/up/outside.txt"] {
        let (head, body) = http_raw(s.addr, p, "").await;
        assert!(head.contains("404"), "{p}: {head}");
        assert!(!body.contains("SECRET") && !body.contains("outside"), "{p}");
    }
    let (head, body) = http_raw(s.addr, "/inside.txt", "").await;
    assert!(
        head.contains("200") && body == "ok",
        "in-dir symlinks still work: {head}"
    );
    let lower = |h: &str| h.to_ascii_lowercase();
    let (head, _) = http_raw(s.addr, "/pkg/a.wasm", "").await;
    assert!(
        lower(&head).contains("content-type: application/wasm"),
        "{head}"
    );
    let (head, _) = http_raw(s.addr, "/manifest.webmanifest", "").await;
    assert!(
        lower(&head).contains("content-type: application/manifest+json"),
        "{head}"
    );
    for p in ["/", "/missing", "/healthz", "/.env"] {
        let (head, _) = http_raw(s.addr, p, "").await;
        let h = lower(&head);
        for needle in [
            "x-content-type-options: nosniff",
            "x-frame-options: deny",
            "referrer-policy: no-referrer",
            "content-security-policy: default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'",
            "connect-src 'self' ws: wss:",
            "worker-src 'self'",
        ] {
            assert!(h.contains(needle), "{p} lacks {needle}: {head}");
        }
    }
    s.shutdown().await;
    let _ = std::fs::remove_dir_all(base);
}

#[tokio::test]
async fn healthz_reports_counts() {
    let s = start(cfg()).await;
    let (status, body) = http_get(s.addr, "/healthz").await;
    assert!(status.contains("200"));
    assert!(body.contains(r#""status":"ok""#) && body.contains(r#""rooms":0"#));
    let (ws, _, _) = create(s.addr).await;
    let (_, body) = http_get(s.addr, "/healthz").await;
    assert!(
        body.contains(r#""rooms":1"#)
            && body.contains(r#""players":1"#)
            && body.contains("tick_p99_us"),
        "{body}"
    );
    drop(ws);
    s.shutdown().await;
}

#[tokio::test]
async fn create_and_join_by_code() {
    let s = start(cfg()).await;
    let (mut a, code, a_id) = create(s.addr).await;
    // Codes are typed by people: case and dashes do not matter.
    let (mut b, b_id) = join(s.addr, &code.to_lowercase().replace('-', " ")).await;
    assert_ne!(a_id, b_id);
    let joined = wait_for(&mut a, |m| matches!(m, ServerMsg::PlayerJoined(_))).await;
    assert!(matches!(joined, ServerMsg::PlayerJoined(p) if p.id == b_id && p.name == "Bo"));
    for ws in [&mut a, &mut b] {
        // The join snapshot may predate the second player for A; the periodic one does not.
        loop {
            let (units, players) = snapshot(ws).await;
            if players.len() == 2 {
                assert!(units.iter().any(|u| u.id == a_id) && units.iter().any(|u| u.id == b_id));
                break;
            }
        }
    }
    let (_, body) = http_get(s.addr, "/healthz").await;
    assert!(body.contains(r#""players":2"#), "{body}");
    s.shutdown().await;
}

#[tokio::test]
async fn ping_emote_and_leave() {
    let s = start(cfg()).await;
    let (mut a, code, _) = create(s.addr).await;
    let (mut b, b_id) = join(s.addr, &code).await;
    a.send(enc(&ClientMsg::Ping { t: 42 })).await.unwrap();
    wait_for(&mut a, |m| *m == ServerMsg::Pong { t: 42 }).await;
    b.send(enc(&ClientMsg::Emote {
        emote: Emote::Lgtm,
        target: None,
    }))
    .await
    .unwrap();
    wait_for(&mut a, |m| {
        *m == ServerMsg::Emoted {
            player: b_id,
            emote: Emote::Lgtm,
            target: None,
        }
    })
    .await;
    b.send(enc(&ClientMsg::LeaveRoom)).await.unwrap();
    assert_eq!(expect_closed(&mut b).await, Some(CloseCode::Normal));
    wait_for(&mut a, |m| *m == ServerMsg::PlayerLeft { id: b_id }).await;
    // remove_player: the unit is gone from the sim.
    let (units, players) = snapshot(&mut a).await;
    assert_eq!(players.len(), 1);
    assert!(units.iter().all(|u| u.id != b_id));
    s.shutdown().await;
}

#[tokio::test]
async fn commands_act_only_as_the_sender() {
    let s = start(cfg()).await;
    let (mut a, code, a_id) = create(s.addr).await;
    let (mut b, b_id) = join(s.addr, &code).await;
    let (units, _) = snapshot(&mut b).await;
    let npc = units
        .iter()
        .find(|u| u.id != a_id && u.id != b_id)
        .unwrap()
        .id;
    // A claims to be B.
    a.send(enc(&ClientMsg::Cmd(Command::SetTarget {
        unit: b_id,
        target: Some(npc),
    })))
    .await
    .unwrap();
    loop {
        let (units, _) = snapshot(&mut b).await;
        let target = |id| units.iter().find(|u| u.id == id).unwrap().target;
        if target(a_id) == Some(npc) {
            assert_eq!(target(b_id), None, "A acted as B");
            break;
        }
    }
    // A non-player command is not even decodable from a client.
    let moved = ClientMsg::Cmd(Command::PlayerMoved {
        pos: Vec3::default(),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    assert!(encode(&moved).is_ok_and(|b| decode::<ClientMsg>(&b).is_err()));
    drop(a);
    s.shutdown().await;
}

#[tokio::test]
async fn teleporting_move_is_corrected() {
    let s = start(cfg()).await;
    let (mut a, _, a_id) = create(s.addr).await;
    let (units, _) = snapshot(&mut a).await;
    let me = units.iter().find(|u| u.id == a_id).unwrap().pos();
    let mv = |pos: Vec3, seq| {
        enc(&ClientMsg::Move {
            pos,
            facing: 0.0,
            moving: true,
            jumping: false,
            seq,
        })
    };
    a.send(mv(Vec3::new(me.x + 0.3, 0.0, me.z), 1))
        .await
        .unwrap();
    a.send(mv(Vec3::new(me.x + 40.0, 0.0, me.z), 2))
        .await
        .unwrap();
    let m = wait_for(&mut a, |m| matches!(m, ServerMsg::MoveCorrection { .. })).await;
    let ServerMsg::MoveCorrection { seq, pos } = m else {
        unreachable!()
    };
    assert_eq!(seq, 2);
    assert!((pos.x - (me.x + 0.3)).abs() < 1e-3, "{pos:?}");
    s.shutdown().await;
}

/// A walking player reaches the others as 10 Hz `Positions`, not only in the 1 Hz snapshot.
#[tokio::test]
async fn remote_moves_arrive_as_positions_at_10hz() {
    let s = start(cfg()).await;
    let (mut a, code, _) = create(s.addr).await;
    let (mut b, b_id) = join(s.addr, &code).await;
    let (units, _) = snapshot(&mut b).await;
    let start = units.iter().find(|u| u.id == b_id).unwrap().pos();
    snapshot(&mut a).await;
    drain_now(&mut a).await;
    let t0 = tokio::time::Instant::now();
    let mut arrivals: Vec<(f32, Vec3)> = Vec::new();
    let mut last_sent = start;
    for seq in 1..=30u32 {
        // 5 m/s in 50 ms steps, along a line that stays near the spawn.
        let k = seq as f32 * 0.25;
        last_sent = Vec3::new(start.x + (k % 3.0) - 1.5, start.y, start.z);
        b.send(enc(&ClientMsg::Move {
            pos: last_sent,
            facing: 0.0,
            moving: true,
            jumping: false,
            seq,
        }))
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        for m in drain_now(&mut a).await {
            if let ServerMsg::Positions { units, .. } = m
                && let Some((_, cm, _)) = units.iter().find(|u| u.0 == b_id)
            {
                arrivals.push((t0.elapsed().as_secs_f32(), net_proto::from_cm(*cm)));
            }
        }
    }
    assert!(
        arrivals.len() >= 10,
        "{} Positions in 1.5 s",
        arrivals.len()
    );
    let gap = arrivals
        .windows(2)
        .map(|w| w[1].0 - w[0].0)
        .fold(0.0f32, f32::max);
    assert!(gap < 0.35, "largest gap {gap} s");
    let (_, last) = arrivals.last().unwrap();
    assert!(
        (last.x - last_sent.x).abs() < 1.0,
        "{last:?} vs {last_sent:?}"
    );
    s.shutdown().await;
}

#[tokio::test]
async fn garbage_frames_strike_out() {
    let s = start(cfg()).await;
    let (mut a, _, _) = create(s.addr).await;
    for _ in 0..5 {
        a.send(Message::Binary(vec![1, 0, 0xff, 0xff].into()))
            .await
            .unwrap();
    }
    let info = close_info(&mut a).await.unwrap();
    assert_eq!(
        info,
        (CloseCode::Policy, "too many invalid messages".to_string())
    );
    // A second CreateRoom inside a room is invalid too.
    let (mut b, _, _) = create(s.addr).await;
    for _ in 0..5 {
        b.send(enc(&ClientMsg::CreateRoom)).await.unwrap();
    }
    assert_eq!(expect_closed(&mut b).await, Some(CloseCode::Policy));
    s.shutdown().await;
}

#[tokio::test]
async fn rate_limit_closes_abusers() {
    let s = start(Config {
        rate_per_sec: 1.0,
        rate_burst: 5.0,
        ..cfg()
    })
    .await;
    let (mut ws, _, _) = create(s.addr).await;
    for _ in 0..50 {
        if ws.send(enc(&ClientMsg::Ping { t: 0 })).await.is_err() {
            break;
        }
    }
    assert_eq!(expect_closed(&mut ws).await, Some(CloseCode::Policy));
    s.shutdown().await;
}

#[tokio::test]
async fn idle_connections_are_closed() {
    let s = start(Config {
        idle_timeout: Duration::from_millis(300),
        ..cfg()
    })
    .await;
    let (mut ws, _, _) = create(s.addr).await;
    assert_eq!(expect_closed(&mut ws).await, Some(CloseCode::Away));
    s.shutdown().await;
}

#[tokio::test]
async fn ping_frames_do_not_keep_a_connection_alive() {
    let s = start(Config {
        idle_timeout: Duration::from_millis(400),
        ..cfg()
    })
    .await;
    let (mut ws, _, _) = create(s.addr).await;
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let closed = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            tokio::select! {
                _ = tick.tick() => { let _ = ws.send(Message::Ping(Vec::new().into())).await; }
                m = ws.next() => match m {
                    Some(Ok(Message::Close(f))) => return f.map(|f| f.code),
                    Some(Ok(_)) => {}
                    _ => return None,
                },
            }
        }
    })
    .await
    .expect("idle close despite pings");
    assert_eq!(closed, Some(CloseCode::Away));
    s.shutdown().await;
}

#[tokio::test]
async fn forwarded_for_connections_are_capped_per_client_ip() {
    let s = start(Config {
        per_ip_conns: 2,
        trust_forwarded_for: true,
        // Hand-built: parsing would clear this, the handler must not rely on that alone.
        lan_exempt: true,
        ..cfg()
    })
    .await;
    let xff = |ip: &'static str| [("x-forwarded-for", ip)];
    let _a = connect_with(s.addr, "", &xff("203.0.113.1")).await.unwrap();
    let _b = connect_with(s.addr, "", &xff("203.0.113.1")).await.unwrap();
    assert!(connect_with(s.addr, "", &xff("203.0.113.1")).await.is_err());
    // Another client behind the same proxy is unaffected; a private hop is not exempt.
    let _c = connect_with(s.addr, "", &xff("203.0.113.2")).await.unwrap();
    let _p1 = connect_with(s.addr, "", &xff("10.0.0.5")).await.unwrap();
    let _p2 = connect_with(s.addr, "", &xff("10.0.0.5")).await.unwrap();
    assert!(connect_with(s.addr, "", &xff("10.0.0.5")).await.is_err());
    // Dropping a connection frees its slot.
    drop(_a);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(connect_with(s.addr, "", &xff("203.0.113.1")).await.is_ok());
    s.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn binary_exits_zero_on_sigterm() {
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_the-last-dev-server"))
        .args(["--bind", "127.0.0.1:0", "--web-dir", "/nonexistent-web-dir"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    // The handler is installed shortly after start; a signal before that kills the process.
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    let sent = std::process::Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
    let status = tokio::task::spawn_blocking(move || child.wait().unwrap())
        .await
        .unwrap();
    assert_eq!(status.code(), Some(0));
}

#[tokio::test]
async fn hello_is_required_in_time() {
    let s = start(Config {
        handshake_timeout: Duration::from_millis(200),
        ..cfg()
    })
    .await;
    let mut silent = connect(s.addr, "").await;
    assert_eq!(
        join_refusal(&mut silent).await,
        (
            ErrorKind::HelloRequired,
            Some((CloseCode::Policy, "handshake timeout".into()))
        )
    );
    let mut rude = connect(s.addr, "").await;
    rude.send(enc(&ClientMsg::CreateRoom)).await.unwrap();
    assert_eq!(join_refusal(&mut rude).await.0, ErrorKind::HelloRequired);
    let mut old = connect(s.addr, "").await;
    old.send(enc(&ClientMsg::Hello {
        version: PROTOCOL_VERSION + 1,
        name: "x".into(),
        class: "fullstack".into(),
    }))
    .await
    .unwrap();
    assert_eq!(join_refusal(&mut old).await.0, ErrorKind::BadVersion);
    let mut rockstar = connect(s.addr, "").await;
    rockstar
        .send(enc(&ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            name: "x".into(),
            class: "10x_rockstar".into(),
        }))
        .await
        .unwrap();
    assert_eq!(join_refusal(&mut rockstar).await.0, ErrorKind::UnknownClass);
    // Hello but no room request in time.
    let mut slow = connect(s.addr, "").await;
    slow.send(hello_msg("x")).await.unwrap();
    assert_eq!(join_refusal(&mut slow).await.0, ErrorKind::HelloRequired);
    s.shutdown().await;
}

#[tokio::test]
async fn connection_cap_and_unknown_room() {
    let s = start(Config {
        max_conns: 1,
        ..cfg()
    })
    .await;
    let (first, _, _) = create(s.addr).await;
    let err = tokio_tungstenite::connect_async(format!("ws://{}/ws", s.addr))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("503"), "{err}");
    drop(first);
    s.shutdown().await;
}

#[tokio::test]
async fn full_unknown_and_malformed_rooms_look_identical() {
    let s = start(Config {
        max_players_per_room: 1,
        ..cfg()
    })
    .await;
    let (_owner, code, _) = create(s.addr).await;
    let mut seen = Vec::new();
    for c in [code.as_str(), "MERGE-CONFLICT-ZZZZZZZZ", "a b"] {
        let mut ws = request_join(s.addr, c).await;
        seen.push(join_refusal(&mut ws).await);
    }
    assert!(seen.windows(2).all(|w| w[0] == w[1]), "{seen:?}");
    assert_eq!(
        seen[0],
        (
            ErrorKind::CannotJoin,
            Some((CloseCode::Policy, "cannot join".into()))
        )
    );
    s.shutdown().await;
}

#[tokio::test]
async fn shutdown_closes_open_sockets() {
    let s = start(cfg()).await;
    let (mut ws, _, _) = create(s.addr).await;
    s.shutdown().await;
    assert_eq!(expect_closed(&mut ws).await, Some(CloseCode::Again));
}

#[tokio::test]
async fn failed_join_lockout() {
    let s = start(Config {
        per_ip_failed_joins: 3,
        failed_join_lockout: Duration::from_secs(30),
        max_players_per_room: 1,
        ..strict()
    })
    .await;
    let (owner, code, _) = create(s.addr).await;
    for c in [code.as_str(), "MERGE-CONFLICT-ZZZZZZZZ", "a b"] {
        let mut ws = request_join(s.addr, c).await;
        assert_eq!(join_refusal(&mut ws).await.0, ErrorKind::CannotJoin);
    }
    // Three failures: even a correct code is refused now (room has space again).
    drop(owner);
    let (_other, _, _) = create(s.addr).await; // creating is still allowed
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut ws = request_join(s.addr, &code).await;
    assert_eq!(join_refusal(&mut ws).await.0, ErrorKind::CannotJoin);
    s.shutdown().await;
}

#[tokio::test]
async fn room_creation_rate_limit() {
    let s = start(Config {
        per_ip_rooms_per_min: 2,
        ..strict()
    })
    .await;
    let _keep = [create(s.addr).await, create(s.addr).await];
    let mut third = connect(s.addr, "").await;
    third.send(hello_msg("x")).await.unwrap();
    third.send(enc(&ClientMsg::CreateRoom)).await.unwrap();
    assert_eq!(join_refusal(&mut third).await.0, ErrorKind::RateLimited);
    s.shutdown().await;
}

#[tokio::test]
async fn five_clients_behind_one_ip_share_a_room() {
    // Default per-IP limits, LAN exemption off: a NAT with a full party must work.
    let s = start(strict()).await;
    let (first, code, _) = create(s.addr).await;
    let mut all = vec![first];
    for _ in 0..4 {
        all.push(join(s.addr, &code).await.0);
    }
    let (_, body) = http_get(s.addr, "/healthz").await;
    assert!(body.contains(r#""players":5"#), "{body}");
    let mut sixth = request_join(s.addr, &code).await;
    assert_eq!(join_refusal(&mut sixth).await.0, ErrorKind::CannotJoin);
    s.shutdown().await;
}

#[tokio::test]
async fn trusted_proxy_forwarded_for_is_used_for_limits() {
    let refused = |mut ws: Ws| async move {
        ws.send(hello_msg("x")).await.unwrap();
        ws.send(enc(&ClientMsg::CreateRoom)).await.unwrap();
        join_refusal(&mut ws).await.0
    };
    let s = start(Config {
        per_ip_rooms_per_min: 1,
        trusted_proxies: vec!["127.0.0.1".parse().unwrap()],
        ..strict()
    })
    .await;
    let _a = create_with(s.addr, &[("x-forwarded-for", "203.0.113.1")]).await;
    // A different forwarded client has its own budget.
    let _b = create_with(s.addr, &[("x-forwarded-for", "203.0.113.2")]).await;
    // Same client again is limited; spoofed left-most entry does not help.
    let c = connect_with(s.addr, "", &[("x-forwarded-for", "9.9.9.9, 203.0.113.1")])
        .await
        .unwrap();
    assert_eq!(refused(c).await, ErrorKind::RateLimited);
    s.shutdown().await;

    // Without a trusted proxy the header is ignored: both share the peer's budget.
    let s = start(Config {
        per_ip_rooms_per_min: 1,
        ..strict()
    })
    .await;
    let _a = create_with(s.addr, &[("x-forwarded-for", "203.0.113.1")]).await;
    let b = connect_with(s.addr, "", &[("x-forwarded-for", "203.0.113.2")])
        .await
        .unwrap();
    assert_eq!(refused(b).await, ErrorKind::RateLimited);
    s.shutdown().await;
}

// ---- scripted play ----

fn yaw_towards(from: Vec3, to: Vec3) -> f32 {
    (-(to.x - from.x)).atan2(-(to.z - from.z))
}

fn dist_xz(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// Walks towards a target along nav paths, then stands and casts.
struct Bot {
    id: EntityId,
    pos: Vec3,
    seq: u32,
    path: Vec<Vec3>,
    replan_in: u32,
    tick: u32,
}

const CAST_RANGE: f32 = 24.0;
const STEP_M: f32 = 6.5 / 30.0;

impl Bot {
    fn new(id: EntityId, units: &[UnitSnap]) -> Self {
        let pos = units.iter().find(|u| u.id == id).unwrap().pos();
        Self {
            id,
            pos,
            seq: 0,
            path: Vec::new(),
            replan_in: 0,
            tick: 0,
        }
    }

    /// One 1/30 s step: what to send.
    fn step(&mut self, nav: &Sim, blockers: &[Aabb], target: Vec3) -> Vec<ClientMsg> {
        self.tick += 1;
        let eye = |p: Vec3| Vec3::new(p.x, 1.5, p.z);
        let in_reach = dist_xz(self.pos, target) <= CAST_RANGE
            && line_of_sight(eye(self.pos), eye(target), blockers);
        let mut out = Vec::new();
        if in_reach {
            if self.tick.is_multiple_of(6) {
                self.seq += 1;
                out.push(ClientMsg::Move {
                    pos: self.pos,
                    facing: yaw_towards(self.pos, target),
                    moving: false,
                    jumping: false,
                    seq: self.seq,
                });
            }
            return out;
        }
        if self.replan_in == 0 || self.path.is_empty() {
            self.path = nav
                .find_path(self.pos, target)
                .unwrap_or_else(|| vec![target]);
            self.replan_in = 30;
        }
        self.replan_in -= 1;
        let mut left = STEP_M;
        while left > 0.0 {
            let Some(&next) = self.path.first() else {
                break;
            };
            let d = dist_xz(self.pos, next);
            if d <= left {
                self.pos = Vec3::new(next.x, 0.0, next.z);
                self.path.remove(0);
                left -= d;
            } else {
                let f = left / d;
                self.pos = Vec3::new(
                    self.pos.x + (next.x - self.pos.x) * f,
                    0.0,
                    self.pos.z + (next.z - self.pos.z) * f,
                );
                left = 0.0;
            }
        }
        self.seq += 1;
        out.push(ClientMsg::Move {
            pos: self.pos,
            facing: yaw_towards(self.pos, target),
            moving: true,
            jumping: false,
            seq: self.seq,
        });
        out
    }
}

#[tokio::test]
async fn two_players_kill_a_campus_mob() {
    let world = World::embedded().unwrap();
    let mut nav = Sim::new(world.content.clone(), 1);
    world.load(&mut nav, "campus", None).unwrap();
    let blockers = &world.zone("campus").unwrap().blockers;
    let hostile = |u: &UnitSnap| {
        u.has(UnitSnap::ALIVE) && world.content.units[usize::from(u.key_idx)].hostile
    };

    let s = start(cfg()).await;
    let (mut a, code, a_id) = create(s.addr).await;
    let (mut b, b_id) = join(s.addr, &code).await;
    let (units, _) = loop {
        let snap = snapshot(&mut a).await;
        if snap.1.len() == 2 {
            break snap;
        }
    };
    let mut bots = [Bot::new(a_id, &units), Bot::new(b_id, &units)];
    let me = bots[0].pos;
    let target = units
        .iter()
        .filter(|u| hostile(u))
        .min_by(|x, y| dist_xz(x.pos(), me).total_cmp(&dist_xz(y.pos(), me)))
        .unwrap()
        .id;
    let mut target_pos = units.iter().find(|u| u.id == target).unwrap().pos();
    let mut died = [false, false];
    let mut corrections = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    let mut ticker = tokio::time::interval(Duration::from_millis(33));
    for (bot, ws) in bots.iter().zip([&mut a, &mut b]) {
        let pick = ClientMsg::Cmd(Command::SetTarget {
            unit: bot.id,
            target: Some(target),
        });
        ws.send(enc(&pick)).await.unwrap();
    }
    while died != [true, true] {
        assert!(
            tokio::time::Instant::now() < deadline,
            "mob never died (corrections {corrections})"
        );
        ticker.tick().await;
        for (k, ws) in [&mut a, &mut b].into_iter().enumerate() {
            for m in drain_now(ws).await {
                match m {
                    ServerMsg::Snapshot { units, .. } => {
                        if let Some(u) = units.iter().find(|u| u.id == target) {
                            target_pos = u.pos();
                        }
                    }
                    ServerMsg::Events { events, .. } => {
                        for e in events {
                            match e {
                                Event::Death { unit, .. } if unit == target => died[k] = true,
                                Event::Death { unit, .. } => {
                                    assert!(unit != a_id && unit != b_id, "a player died")
                                }
                                _ => {}
                            }
                        }
                    }
                    ServerMsg::MoveCorrection { pos, .. } => {
                        corrections += 1;
                        bots[k].pos = pos;
                        bots[k].path.clear();
                    }
                    _ => {}
                }
            }
        }
        for (k, ws) in [&mut a, &mut b].into_iter().enumerate() {
            let mut msgs = bots[k].step(&nav, blockers, target_pos);
            if bots[k].tick.is_multiple_of(15) {
                // Casts while walking just fail; standing still they land.
                msgs.push(ClientMsg::Cmd(Command::SetTarget {
                    unit: bots[k].id,
                    target: Some(target),
                }));
                msgs.push(ClientMsg::Cmd(Command::Cast {
                    caster: bots[k].id,
                    spell: SpellId(1),
                }));
            }
            for m in msgs {
                ws.send(enc(&m)).await.unwrap();
            }
        }
    }
    s.shutdown().await;
}

// ---- load ----

/// `cargo test -p server --release -- --ignored load --nocapture`
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "load test, 30 s"]
async fn load_10_rooms_of_5_bots() {
    const ROOMS: usize = 10;
    const BOTS: usize = 5;
    const SECS: u64 = 30;
    let s = start(Config {
        max_conns: 100,
        ..cfg()
    })
    .await;
    let addr = s.addr;
    let mut tasks = Vec::new();
    for _ in 0..ROOMS {
        let (first, code, id) = create(addr).await;
        let mut members = vec![(first, id)];
        for _ in 1..BOTS {
            members.push(join(addr, &code).await);
        }
        for (k, (mut ws, id)) in members.into_iter().enumerate() {
            tasks.push(tokio::spawn(async move {
                let mut bytes = 0usize;
                let mut seq = 0u32;
                let mut center: Option<Vec3> = None;
                let mut ticker = tokio::time::interval(Duration::from_millis(33));
                let end = tokio::time::Instant::now() + Duration::from_secs(SECS);
                while tokio::time::Instant::now() < end {
                    ticker.tick().await;
                    while let Ok(Some(Ok(m))) =
                        tokio::time::timeout(Duration::from_millis(1), ws.next()).await
                    {
                        bytes += m.len();
                        if let (None, Message::Binary(b)) = (center, &m)
                            && let Ok(ServerMsg::Snapshot { units, .. }) = decode(b)
                        {
                            center = units.iter().find(|u| u.id == id).map(UnitSnap::pos);
                        }
                    }
                    let Some(c) = center else { continue };
                    seq += 1;
                    // A slow circle (3 m/s) around the spawn point.
                    let t = seq as f32 / 30.0 + k as f32;
                    let pos = Vec3::new(c.x + t.cos() - 1.0, 0.0, c.z + t.sin());
                    ws.send(enc(&ClientMsg::Move {
                        pos,
                        facing: t,
                        moving: true,
                        jumping: false,
                        seq,
                    }))
                    .await
                    .unwrap();
                    if seq.is_multiple_of(30) {
                        ws.send(enc(&ClientMsg::Cmd(Command::SetTarget {
                            unit: id,
                            target: None,
                        })))
                        .await
                        .unwrap();
                    }
                }
                bytes
            }));
        }
    }
    // Sampled near the end so the p99 covers steady state.
    tokio::time::sleep(Duration::from_secs(SECS - 2)).await;
    let (_, body) = http_get(addr, "/healthz").await;
    let mut total = 0usize;
    for t in tasks {
        total += t.await.unwrap();
    }
    let per_client = total as f64 / (ROOMS * BOTS) as f64 / SECS as f64;
    println!("load: {ROOMS} rooms x {BOTS} bots, {SECS} s");
    println!("load: healthz {body}");
    println!("load: {per_client:.0} bytes/s per client (down)");
    assert!(
        body.contains(&format!(r#""players":{}"#, ROOMS * BOTS)),
        "{body}"
    );
    s.shutdown().await;
}

/// Career Ladder online: a pick travels as a command, the server sim validates it against the
/// level, everyone gets `TalentsChanged`, and snapshots carry the picks for late joiners.
#[tokio::test]
async fn talent_picks_are_validated_and_shared() {
    let s = start(Config {
        dev_start_level: vec![4],
        ..cfg()
    })
    .await;
    let (mut a, code, a_id) = create(s.addr).await;
    let (mut b, _) = join(s.addr, &code).await;
    // Row 2 needs level 6: refused, only the sender hears why.
    a.send(enc(&ClientMsg::Cmd(Command::PickTalent {
        row: 2,
        choice: 1,
    })))
    .await
    .unwrap();
    wait_for(&mut a, |m| {
        matches!(m, ServerMsg::Events { events, .. } if events.iter().any(|e| matches!(e,
            Event::ItemError { reason: combat_core::ItemError::NotEligible })))
    })
    .await;
    a.send(enc(&ClientMsg::Cmd(Command::PickTalent {
        row: 1,
        choice: 3,
    })))
    .await
    .unwrap();
    wait_for(&mut b, |m| {
        matches!(m, ServerMsg::Events { events, .. } if events.contains(&Event::TalentsChanged {
            unit: a_id,
            talents: [0, 3, 0, 0, 0],
        }))
    })
    .await;
    wait_for(&mut b, |m| {
        matches!(m, ServerMsg::Snapshot { players, .. }
            if players.iter().any(|p| p.id == a_id && p.talents == [0, 3, 0, 0, 0]))
    })
    .await;
    s.shutdown().await;
}
