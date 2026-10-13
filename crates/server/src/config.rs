//! Configuration from environment variables, overridden by command-line flags.
//! No argument-parsing crate: the flag set is tiny and fixed.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub web_dir: PathBuf,
    /// Content file instead of the embedded `content/game.ron`; must match the clients' build.
    pub content: Option<PathBuf>,
    pub max_rooms: usize,
    pub max_players_per_room: usize,
    pub max_conns: usize,
    /// Hard cap per WebSocket message; also bounds per-connection memory.
    pub max_message_bytes: usize,
    /// Messages per second a connection may sustain.
    pub rate_per_sec: f64,
    pub rate_burst: f64,
    /// Close a connection that sent nothing (not even a ping) for this long.
    pub idle_timeout: Duration,
    /// An empty room that had several players is destroyed after this long.
    pub empty_room_ttl: Duration,
    /// An empty room whose creator never had company expires much sooner (creation flooding).
    pub unjoined_room_ttl: Duration,
    /// Outbound queue per player; a client that lets it fill up is dropped.
    pub outbound_queue: usize,
    /// Extra browser origins allowed to open `/ws` (same-origin and origin-less clients always are).
    pub allowed_origins: Vec<String>,
    /// Reverse proxies whose `X-Forwarded-For` is believed.
    pub trusted_proxies: Vec<IpAddr>,
    /// Believe the right-most `X-Forwarded-For` entry from any peer. Only for platforms whose
    /// proxy IPs are not fixed (Render) and where the server is reachable solely through that
    /// proxy; otherwise any client could pick its own IP for the per-IP limits.
    pub trust_forwarded_for: bool,
    pub healthz_details: bool,
    /// Max time to receive a full HTTP request head (slowloris).
    pub header_read_timeout: Duration,
    /// The first binary frame must arrive within this long after the upgrade.
    pub handshake_timeout: Duration,
    /// Invalid messages tolerated per connection before it is closed.
    pub invalid_message_limit: u32,
    // Per-IP limits; 0 disables. Defaults are generous because a NAT shares one IP among many players.
    pub per_ip_conns: usize,
    pub per_ip_rooms_per_min: u32,
    pub per_ip_failed_joins: u32,
    pub failed_join_lockout: Duration,
    /// Loopback/private-range clients skip per-IP limits (LAN play); global caps still apply.
    pub lan_exempt: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: SocketAddr::from(([0, 0, 0, 0], 7777)),
            web_dir: PathBuf::from("./web"),
            content: None,
            max_rooms: 50,
            max_players_per_room: 5,
            max_conns: 200,
            max_message_bytes: 64 * 1024,
            rate_per_sec: 60.0,
            rate_burst: 120.0,
            idle_timeout: Duration::from_secs(30),
            empty_room_ttl: Duration::from_secs(600),
            unjoined_room_ttl: Duration::from_secs(60),
            outbound_queue: 64,
            allowed_origins: Vec::new(),
            trusted_proxies: Vec::new(),
            trust_forwarded_for: false,
            healthz_details: false,
            header_read_timeout: Duration::from_secs(10),
            handshake_timeout: Duration::from_secs(2),
            invalid_message_limit: 5,
            per_ip_conns: 16,
            per_ip_rooms_per_min: 6,
            per_ip_failed_joins: 10,
            failed_join_lockout: Duration::from_secs(30),
            lan_exempt: true,
        }
    }
}

pub const HELP: &str = "the-last-dev-server

USAGE: the-last-dev-server [--bind ADDR] [--web-dir DIR] [--content FILE] [--max-rooms N]
                           [--max-players-per-room N] [--max-conns N]
                           [--allowed-origin ORIGIN]... [--trusted-proxy IP]...
                           [--trust-forwarded-for] [--healthz-details] [--per-ip-conns N]
                           [--per-ip-rooms-per-min N] [--per-ip-failed-joins N]
                           [--no-lan-exempt]

Each flag also has an environment variable: TLD_BIND, TLD_WEB_DIR, TLD_CONTENT,
TLD_MAX_ROOMS, TLD_MAX_PLAYERS_PER_ROOM, TLD_MAX_CONNS, TLD_ALLOWED_ORIGINS
(comma list), TLD_TRUSTED_PROXIES (comma list), TLD_TRUST_FORWARDED_FOR=1,
TLD_HEALTHZ_DETAILS=1, TLD_PER_IP_CONNS, TLD_PER_IP_ROOMS_PER_MIN,
TLD_PER_IP_FAILED_JOINS, TLD_NO_LAN_EXEMPT=1. Flags win. Per-IP limits: 0 disables.
Without --bind/TLD_BIND, PORT (set by hosts like Render) binds 0.0.0.0:$PORT.
Defaults: 0.0.0.0:7777, ./web, embedded content, 50, 5, 200, per-IP 16 / 6 / 10.";

fn truthy(v: &str) -> bool {
    matches!(v, "1" | "true" | "yes" | "on")
}

impl Config {
    /// `Ok(None)` means help was requested.
    pub fn from_env_and_args() -> Result<Option<Self>, String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        Self::parse(|k| std::env::var(k).ok(), &args)
    }

    pub fn parse(
        env: impl Fn(&str) -> Option<String>,
        args: &[String],
    ) -> Result<Option<Self>, String> {
        let mut c = Self::default();
        if let Some(port) = env("PORT") {
            let port: u16 = port
                .trim()
                .parse()
                .map_err(|_| "invalid value for PORT: expected a port number".to_string())?;
            c.bind = SocketAddr::from(([0, 0, 0, 0], port));
        }
        let apply = |c: &mut Self, flag: &str, v: &str| -> Result<(), String> {
            let bad = |what: &str| format!("invalid value for {flag}: {what}");
            match flag {
                "--bind" => c.bind = v.parse().map_err(|_| bad("expected host:port"))?,
                "--web-dir" => c.web_dir = PathBuf::from(v),
                "--content" => c.content = Some(PathBuf::from(v)),
                "--max-rooms" => c.max_rooms = v.parse().map_err(|_| bad("expected integer"))?,
                "--max-players-per-room" => {
                    c.max_players_per_room = v.parse().map_err(|_| bad("expected integer"))?
                }
                "--max-conns" => c.max_conns = v.parse().map_err(|_| bad("expected integer"))?,
                "--per-ip-conns" => {
                    c.per_ip_conns = v.parse().map_err(|_| bad("expected integer"))?
                }
                "--per-ip-rooms-per-min" => {
                    c.per_ip_rooms_per_min = v.parse().map_err(|_| bad("expected integer"))?
                }
                "--per-ip-failed-joins" => {
                    c.per_ip_failed_joins = v.parse().map_err(|_| bad("expected integer"))?
                }
                "--allowed-origin" => {
                    for o in v.split(',').map(str::trim).filter(|o| !o.is_empty()) {
                        if !o.contains("://") {
                            return Err(bad("expected scheme://host[:port]"));
                        }
                        c.allowed_origins
                            .push(o.trim_end_matches('/').to_ascii_lowercase());
                    }
                }
                "--trusted-proxy" => {
                    for ip in v.split(',').map(str::trim).filter(|o| !o.is_empty()) {
                        c.trusted_proxies
                            .push(ip.parse().map_err(|_| bad("expected IP address"))?);
                    }
                }
                _ => return Err(format!("unknown flag {flag}")),
            }
            Ok(())
        };
        for (var, flag) in [
            ("TLD_BIND", "--bind"),
            ("TLD_WEB_DIR", "--web-dir"),
            ("TLD_CONTENT", "--content"),
            ("TLD_MAX_ROOMS", "--max-rooms"),
            ("TLD_MAX_PLAYERS_PER_ROOM", "--max-players-per-room"),
            ("TLD_MAX_CONNS", "--max-conns"),
            ("TLD_PER_IP_CONNS", "--per-ip-conns"),
            ("TLD_PER_IP_ROOMS_PER_MIN", "--per-ip-rooms-per-min"),
            ("TLD_PER_IP_FAILED_JOINS", "--per-ip-failed-joins"),
            ("TLD_ALLOWED_ORIGINS", "--allowed-origin"),
            ("TLD_TRUSTED_PROXIES", "--trusted-proxy"),
        ] {
            if let Some(v) = env(var) {
                apply(&mut c, flag, &v)?;
            }
        }
        c.healthz_details = env("TLD_HEALTHZ_DETAILS").is_some_and(|v| truthy(&v));
        c.trust_forwarded_for = env("TLD_TRUST_FORWARDED_FOR").is_some_and(|v| truthy(&v));
        c.lan_exempt = !env("TLD_NO_LAN_EXEMPT").is_some_and(|v| truthy(&v));

        let mut cli_lists = Vec::new();
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            match flag.as_str() {
                "--help" | "-h" => return Ok(None),
                "--healthz-details" => {
                    c.healthz_details = true;
                    continue;
                }
                "--no-lan-exempt" => {
                    c.lan_exempt = false;
                    continue;
                }
                "--trust-forwarded-for" => {
                    c.trust_forwarded_for = true;
                    continue;
                }
                _ => {}
            }
            // Accept both `--flag value` and `--flag=value`.
            let (f, v) = match flag.split_once('=') {
                Some((f, v)) => (f, v.to_string()),
                None => (
                    flag.as_str(),
                    it.next()
                        .ok_or_else(|| format!("missing value for {flag}"))?
                        .clone(),
                ),
            };
            // A list given on the command line replaces the env-provided one.
            if !cli_lists.contains(&f.to_string()) {
                match f {
                    "--allowed-origin" => c.allowed_origins.clear(),
                    "--trusted-proxy" => c.trusted_proxies.clear(),
                    _ => {}
                }
                cli_lists.push(f.to_string());
            }
            apply(&mut c, f, &v)?;
        }
        // A private right-most X-Forwarded-For hop would otherwise exempt every client.
        if c.trust_forwarded_for {
            c.lan_exempt = false;
        }
        Ok(Some(c))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn flags_override_env() {
        let env = |k: &str| (k == "TLD_MAX_ROOMS").then(|| "7".to_string());
        let c = Config::parse(env, &s(&["--max-rooms=9", "--bind", "127.0.0.1:1"]))
            .unwrap()
            .unwrap();
        assert_eq!(c.max_rooms, 9);
        assert_eq!(c.bind.port(), 1);
        assert!(c.content.is_none());
        let c = Config::parse(|_| None, &s(&["--content", "x.ron"]))
            .unwrap()
            .unwrap();
        assert_eq!(c.content.as_deref(), Some(std::path::Path::new("x.ron")));
    }

    #[test]
    fn port_env_and_forwarded_for_trust() {
        let env = |k: &str| match k {
            "PORT" => Some("10000".to_string()),
            "TLD_TRUST_FORWARDED_FOR" => Some("1".to_string()),
            _ => None,
        };
        let c = Config::parse(env, &[]).unwrap().unwrap();
        assert_eq!(c.bind, SocketAddr::from(([0, 0, 0, 0], 10000)));
        assert!(c.trust_forwarded_for);
        // An explicit bind wins over PORT.
        let both = |k: &str| match k {
            "PORT" => Some("10000".to_string()),
            "TLD_BIND" => Some("127.0.0.1:9".to_string()),
            _ => None,
        };
        assert_eq!(Config::parse(both, &[]).unwrap().unwrap().bind.port(), 9);
        let c = Config::parse(env, &s(&["--bind", "127.0.0.1:8"]))
            .unwrap()
            .unwrap();
        assert_eq!(c.bind.port(), 8);
        let c = Config::parse(|_| None, &s(&["--trust-forwarded-for"]))
            .unwrap()
            .unwrap();
        assert!(c.trust_forwarded_for && c.bind.port() == 7777);
        assert!(
            !Config::parse(|_| None, &[])
                .unwrap()
                .unwrap()
                .trust_forwarded_for
        );
        let bad = |k: &str| (k == "PORT").then(|| "http".to_string());
        assert!(Config::parse(bad, &[]).is_err());
    }

    #[test]
    fn trusting_forwarded_for_disables_lan_exemption() {
        let env = |k: &str| (k == "TLD_TRUST_FORWARDED_FOR").then(|| "1".to_string());
        assert!(!Config::parse(env, &[]).unwrap().unwrap().lan_exempt);
        let c = Config::parse(|_| None, &s(&["--trust-forwarded-for"]))
            .unwrap()
            .unwrap();
        assert!(!c.lan_exempt);
        assert!(Config::parse(|_| None, &[]).unwrap().unwrap().lan_exempt);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Config::parse(|_| None, &s(&["--nope", "1"])).is_err());
        assert!(Config::parse(|_| None, &s(&["--max-conns", "x"])).is_err());
        assert!(Config::parse(|_| None, &s(&["--max-conns"])).is_err());
    }

    #[test]
    fn lists_flags_and_defaults() {
        let env = |k: &str| match k {
            "TLD_ALLOWED_ORIGINS" => Some("https://a.example,https://b.example/".to_string()),
            "TLD_PER_IP_CONNS" => Some("3".to_string()),
            _ => None,
        };
        let c = Config::parse(env, &[]).unwrap().unwrap();
        assert_eq!(
            c.allowed_origins,
            ["https://a.example", "https://b.example"]
        );
        assert_eq!(
            (
                c.per_ip_conns,
                c.per_ip_rooms_per_min,
                c.per_ip_failed_joins
            ),
            (3, 6, 10)
        );
        assert!(c.lan_exempt && !c.healthz_details);
        let c = Config::parse(
            env,
            &s(&[
                "--allowed-origin=HTTPS://Pages.Example",
                "--allowed-origin",
                "https://x.example",
                "--trusted-proxy",
                "10.0.0.1,::1",
                "--healthz-details",
                "--no-lan-exempt",
                "--per-ip-conns=0",
            ]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            c.allowed_origins,
            ["https://pages.example", "https://x.example"]
        );
        assert_eq!(c.trusted_proxies.len(), 2);
        assert!(c.healthz_details && !c.lan_exempt && c.per_ip_conns == 0);
        assert!(Config::parse(|_| None, &s(&["--allowed-origin", "nohost"])).is_err());
        assert!(Config::parse(|_| None, &s(&["--trusted-proxy", "nope"])).is_err());
    }
}
