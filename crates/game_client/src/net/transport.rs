//! Protocol-agnostic binary websocket transport. Nothing here knows about game messages and
//! nothing runs until `NetClient::connect` is called (offline play never touches it).

use bevy::platform::time::Instant;
use bevy::prelude::*;
use std::time::Duration;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const UNREACHABLE: &str = "Server unreachable";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum NetState {
    #[default]
    Disconnected,
    Connecting {
        since: Instant,
    },
    Connected,
    Failed {
        reason: String,
    },
}

/// Written as a Bevy message by `pump_net`, so protocol code can read them with a `MessageReader`.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub enum NetEvent {
    Opened,
    Frame(Vec<u8>),
    Closed { reason: String },
    Error(String),
}

impl NetState {
    /// Pure transition for one transport event; events after a failure change nothing.
    pub fn apply(&self, event: &NetEvent) -> NetState {
        let live = matches!(self, NetState::Connecting { .. } | NetState::Connected);
        match event {
            NetEvent::Opened if matches!(self, NetState::Connecting { .. }) => NetState::Connected,
            NetEvent::Closed { reason } | NetEvent::Error(reason) if live => NetState::Failed {
                reason: reason.clone(),
            },
            _ => self.clone(),
        }
    }

    pub fn timed_out(&self, elapsed: Duration) -> bool {
        matches!(self, NetState::Connecting { .. }) && elapsed >= CONNECT_TIMEOUT
    }
}

/// Dropping the client closes the socket.
#[derive(Resource, Default)]
pub struct NetClient {
    state: NetState,
    conn: Option<backend::Conn>,
}

impl NetClient {
    pub fn state(&self) -> &NetState {
        &self.state
    }

    /// `url` is a full `ws://` or `wss://` URL (see `address::normalize`). Replaces any open connection.
    pub fn connect(&mut self, url: &str) {
        self.conn = None;
        match backend::open(url) {
            Ok(conn) => {
                self.conn = Some(conn);
                self.state = NetState::Connecting {
                    since: Instant::now(),
                };
            }
            Err(reason) => self.state = NetState::Failed { reason },
        }
    }

    pub fn disconnect(&mut self) {
        self.conn = None;
        self.state = NetState::Disconnected;
    }

    /// False when there is no open connection to send on.
    pub fn send(&self, frame: Vec<u8>) -> bool {
        self.state == NetState::Connected && self.conn.as_ref().is_some_and(|c| c.send(frame))
    }

    /// Drains pending transport events and advances `state`. Call once per frame.
    pub fn poll(&mut self) -> Vec<NetEvent> {
        let mut events = self
            .conn
            .as_ref()
            .map(backend::Conn::drain)
            .unwrap_or_default();
        for e in &events {
            self.state = self.state.apply(e);
        }
        if let NetState::Connecting { since } = self.state
            && self.state.timed_out(since.elapsed())
        {
            events.push(NetEvent::Error(UNREACHABLE.to_string()));
            self.state = NetState::Failed {
                reason: UNREACHABLE.to_string(),
            };
        }
        if matches!(self.state, NetState::Failed { .. }) {
            self.conn = None;
        }
        events
    }
}

pub fn pump_net(mut client: ResMut<NetClient>, mut out: MessageWriter<NetEvent>) {
    // Skip the mutable borrow while offline so change detection stays quiet.
    if client.conn.is_none() && !matches!(client.state, NetState::Connecting { .. }) {
        return;
    }
    for e in client.poll() {
        out.write(e);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use super::{CONNECT_TIMEOUT, NetEvent, UNREACHABLE};
    use std::io::ErrorKind;
    use std::net::{TcpStream, ToSocketAddrs};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tungstenite::{Error, HandshakeError, Message};

    /// Short read timeout so the one thread can interleave reads, writes and the stop flag.
    const TICK: Duration = Duration::from_millis(50);

    pub struct Conn {
        out: Sender<Vec<u8>>,
        // Mutex only because Receiver is not Sync and Bevy resources must be.
        events: Mutex<Receiver<NetEvent>>,
        stop: Arc<AtomicBool>,
    }

    impl Conn {
        pub fn send(&self, frame: Vec<u8>) -> bool {
            self.out.send(frame).is_ok()
        }

        pub fn drain(&self) -> Vec<NetEvent> {
            self.events
                .lock()
                .map(|rx| rx.try_iter().collect())
                .unwrap_or_default()
        }
    }

    impl Drop for Conn {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    pub fn open(url: &str) -> Result<Conn, String> {
        let (out, out_rx) = mpsc::channel();
        let (ev_tx, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_url, thread_stop) = (url.to_string(), stop.clone());
        std::thread::Builder::new()
            .name("net-ws".into())
            .spawn(move || {
                let ev = match session(&thread_url, &ev_tx, &out_rx, &thread_stop) {
                    Ok(reason) => NetEvent::Closed { reason },
                    Err(reason) => NetEvent::Error(reason),
                };
                let _ = ev_tx.send(ev);
            })
            .map_err(|_| "Could not start the network thread.".to_string())?;
        Ok(Conn {
            out,
            events: Mutex::new(events),
            stop,
        })
    }

    fn io_reason(e: &std::io::Error) -> String {
        match e.kind() {
            ErrorKind::ConnectionRefused => {
                format!("{UNREACHABLE}: connection refused, nobody is home.")
            }
            ErrorKind::TimedOut | ErrorKind::WouldBlock => UNREACHABLE.to_string(),
            _ => format!("{UNREACHABLE}: {e}"),
        }
    }

    /// Ok = ended normally (reason for the UI), Err = failure reason.
    fn session(
        url: &str,
        events: &Sender<NetEvent>,
        outgoing: &Receiver<Vec<u8>>,
        stop: &AtomicBool,
    ) -> Result<String, String> {
        let uri: tungstenite::http::Uri = url
            .parse()
            .map_err(|_| "Malformed server address.".to_string())?;
        if uri.scheme_str() == Some("wss") {
            return Err(
                "This build has no TLS for wss://. Use ws:// on a LAN, or play in the browser."
                    .into(),
            );
        }
        let host = uri
            .host()
            .unwrap_or_default()
            .trim_matches(['[', ']'])
            .to_string();
        let port = uri.port_u16().unwrap_or(80);
        let addrs = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|_| "Unknown host: DNS has left the building.".to_string())?;
        let mut last = None;
        let mut stream = None;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(e) => last = Some(e),
            }
        }
        let stream =
            stream.ok_or_else(|| last.as_ref().map_or(UNREACHABLE.to_string(), io_reason))?;
        // Blocking-with-timeout for the handshake, then the short tick for the main loop.
        let _ = stream.set_nodelay(true);
        let _ = stream.set_read_timeout(Some(CONNECT_TIMEOUT));
        let (mut ws, _) = tungstenite::client(url, stream).map_err(|e| match e {
            HandshakeError::Failure(Error::Io(e)) => io_reason(&e),
            _ => "Not a Last Dev server (handshake failed).".to_string(),
        })?;
        let _ = ws.get_mut().set_read_timeout(Some(TICK));
        if events.send(NetEvent::Opened).is_err() {
            return Ok("Client dropped".into());
        }
        loop {
            if stop.load(Ordering::Relaxed) {
                let _ = ws.close(None);
                let _ = ws.flush();
                return Ok("Disconnected".into());
            }
            while let Ok(frame) = outgoing.try_recv() {
                ws.send(Message::Binary(frame.into()))
                    .map_err(|e| e.to_string())?;
            }
            match ws.read() {
                Ok(Message::Binary(b)) => {
                    if events.send(NetEvent::Frame(b.to_vec())).is_err() {
                        return Ok("Client dropped".into());
                    }
                }
                Ok(Message::Close(_)) | Err(Error::ConnectionClosed | Error::AlreadyClosed) => {
                    return Ok("The server closed the connection.".into());
                }
                Ok(_) => {}
                Err(Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(e) => return Err(format!("Connection lost: {e}")),
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod backend {
    use super::NetEvent;
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

    type Queue = Rc<RefCell<VecDeque<NetEvent>>>;

    struct WebSock {
        ws: WebSocket,
        queue: Queue,
        _open: Closure<dyn FnMut()>,
        _message: Closure<dyn FnMut(MessageEvent)>,
        _error: Closure<dyn FnMut(Event)>,
        _close: Closure<dyn FnMut(CloseEvent)>,
    }

    impl Drop for WebSock {
        fn drop(&mut self) {
            // Handlers first: a dropped closure that JS still calls throws.
            self.ws.set_onopen(None);
            self.ws.set_onmessage(None);
            self.ws.set_onerror(None);
            self.ws.set_onclose(None);
            let _ = self.ws.close();
        }
    }

    // wasm is single-threaded and Rc is !Send, so the socket lives here and `Conn` stays a plain handle.
    thread_local! {
        static SOCK: RefCell<Option<WebSock>> = const { RefCell::new(None) };
    }

    pub struct Conn(());

    impl Conn {
        pub fn send(&self, frame: Vec<u8>) -> bool {
            SOCK.with(|s| {
                s.borrow()
                    .as_ref()
                    .is_some_and(|w| w.ws.send_with_u8_array(&frame).is_ok())
            })
        }

        pub fn drain(&self) -> Vec<NetEvent> {
            SOCK.with(|s| {
                s.borrow()
                    .as_ref()
                    .map(|w| w.queue.borrow_mut().drain(..).collect())
                    .unwrap_or_default()
            })
        }
    }

    impl Drop for Conn {
        fn drop(&mut self) {
            let _ = SOCK.try_with(|s| s.try_borrow_mut().map(|mut s| s.take()));
        }
    }

    pub fn open(url: &str) -> Result<Conn, String> {
        let ws = WebSocket::new(url).map_err(|_| "Malformed server address.".to_string())?;
        ws.set_binary_type(BinaryType::Arraybuffer);
        let queue: Queue = Rc::default();
        let opened = Rc::new(Cell::new(false));

        let open = {
            let (q, opened) = (queue.clone(), opened.clone());
            Closure::<dyn FnMut()>::new(move || {
                opened.set(true);
                q.borrow_mut().push_back(NetEvent::Opened);
            })
        };
        let message = {
            let q = queue.clone();
            Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                if let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                    q.borrow_mut()
                        .push_back(NetEvent::Frame(js_sys::Uint8Array::new(&buf).to_vec()));
                }
            })
        };
        let error = {
            let (q, opened) = (queue.clone(), opened.clone());
            Closure::<dyn FnMut(Event)>::new(move |_| {
                let msg = if opened.get() {
                    "Connection error."
                } else {
                    super::UNREACHABLE
                };
                q.borrow_mut().push_back(NetEvent::Error(msg.to_string()));
            })
        };
        let close = {
            let (q, opened) = (queue.clone(), opened);
            Closure::<dyn FnMut(CloseEvent)>::new(move |_| {
                let reason = if opened.get() {
                    "The server closed the connection."
                } else {
                    super::UNREACHABLE
                };
                q.borrow_mut().push_back(NetEvent::Closed {
                    reason: reason.to_string(),
                });
            })
        };
        ws.set_onopen(Some(open.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(message.as_ref().unchecked_ref()));
        ws.set_onerror(Some(error.as_ref().unchecked_ref()));
        ws.set_onclose(Some(close.as_ref().unchecked_ref()));

        let sock = WebSock {
            ws,
            queue,
            _open: open,
            _message: message,
            _error: error,
            _close: close,
        };
        SOCK.with(|s| *s.borrow_mut() = Some(sock));
        Ok(Conn(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connecting() -> NetState {
        NetState::Connecting {
            since: Instant::now(),
        }
    }

    fn closed(r: &str) -> NetEvent {
        NetEvent::Closed { reason: r.into() }
    }

    #[test]
    fn open_connects_only_from_connecting() {
        assert_eq!(connecting().apply(&NetEvent::Opened), NetState::Connected);
        assert_eq!(
            NetState::Disconnected.apply(&NetEvent::Opened),
            NetState::Disconnected
        );
    }

    #[test]
    fn errors_and_closes_fail_live_states() {
        let failed = |r: &str| NetState::Failed { reason: r.into() };
        assert_eq!(
            connecting().apply(&NetEvent::Error("x".into())),
            failed("x")
        );
        assert_eq!(NetState::Connected.apply(&closed("gone")), failed("gone"));
        // A close after a failure keeps the first, more specific reason.
        assert_eq!(failed("x").apply(&closed("y")), failed("x"));
        assert_eq!(
            NetState::Disconnected.apply(&closed("y")),
            NetState::Disconnected
        );
    }

    #[test]
    fn frames_do_not_change_state() {
        assert_eq!(
            NetState::Connected.apply(&NetEvent::Frame(vec![1])),
            NetState::Connected
        );
    }

    #[test]
    fn timeout_applies_only_while_connecting() {
        assert!(!connecting().timed_out(Duration::from_secs(4)));
        assert!(connecting().timed_out(CONNECT_TIMEOUT));
        assert!(!NetState::Connected.timed_out(Duration::from_secs(60)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn refused_connection_fails_without_panic() {
        let mut client = NetClient::default();
        client.connect("ws://127.0.0.1:1/ws");
        let deadline = Instant::now() + Duration::from_secs(8);
        while matches!(client.state(), NetState::Connecting { .. }) && Instant::now() < deadline {
            client.poll();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            matches!(client.state(), NetState::Failed { .. }),
            "{:?}",
            client.state()
        );
        assert!(!client.send(vec![1]));
    }
}
