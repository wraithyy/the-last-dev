//! Per-IP abuse controls: concurrent-connection cap, windowed limiters, client IP resolution.
//! Uses tokio's clock so tests can pause time.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::http::HeaderMap;
use tokio::time::Instant;

/// Bound on tracked IPs so a spoofed-source flood cannot grow memory without limit.
const MAX_TRACKED: usize = 10_000;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // Plain counters; a panic elsewhere cannot leave them inconsistent.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Normalises an address for limiting: IPv6 clients usually own a whole /64, so per-address
/// limits would be trivially bypassed, and v4-mapped addresses must match their v4 form.
pub fn ip_key(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return IpAddr::V4(v4);
            }
            let mut o = v6.octets();
            o[8..].fill(0);
            IpAddr::V6(Ipv6Addr::from(o))
        }
        v4 => v4,
    }
}

/// Loopback, RFC1918, link-local and IPv6 ULA: addresses of a LAN, where per-IP limits
/// would only hurt a household playing together.
pub fn is_lan(ip: IpAddr) -> bool {
    // Not ip_key: masking the low 64 bits would turn ::1 into ::.
    let ip = match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    };
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

/// The real client address: the socket peer, unless the peer is a trusted proxy, in which case
/// the right-most `X-Forwarded-For` entry that is not itself a trusted proxy (earlier entries are
/// client-controlled and spoofable).
pub fn client_ip(peer: IpAddr, headers: &HeaderMap, trusted: &[IpAddr]) -> IpAddr {
    if !trusted.contains(&peer) {
        return peer;
    }
    let hops: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .collect();
    for h in hops.iter().rev() {
        match h.parse::<IpAddr>() {
            Ok(ip) if trusted.contains(&ip) => continue,
            Ok(ip) => return ip,
            // A malformed entry means the chain cannot be trusted; blame the proxy itself.
            Err(_) => return peer,
        }
    }
    peer
}

/// Concurrent connections per IP. A limit of 0 disables it.
pub struct ConnTracker {
    max: usize,
    counts: Mutex<HashMap<IpAddr, usize>>,
}

pub struct ConnGuard {
    tracker: Arc<ConnTracker>,
    key: IpAddr,
}

impl ConnTracker {
    pub fn new(max: usize) -> Arc<Self> {
        Arc::new(Self {
            max,
            counts: Mutex::new(HashMap::new()),
        })
    }

    /// None means the IP is over its cap.
    pub fn acquire(self: &Arc<Self>, ip: IpAddr) -> Option<ConnGuard> {
        let key = ip_key(ip);
        let mut counts = lock(&self.counts);
        let n = counts.entry(key).or_insert(0);
        if self.max != 0 && *n >= self.max {
            return None;
        }
        *n += 1;
        Some(ConnGuard {
            tracker: self.clone(),
            key,
        })
    }
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        let mut counts = lock(&self.tracker.counts);
        if let Some(n) = counts.get_mut(&self.key) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                counts.remove(&self.key);
            }
        }
    }
}

struct Slot {
    start: Instant,
    count: u32,
    locked_until: Option<Instant>,
}

/// `max` events per `window` per IP. With a `lockout`, reaching `max` blocks the IP for that
/// long; without one, it is blocked until the window rolls over. `max == 0` disables it.
pub struct RateLimiter {
    max: u32,
    window: Duration,
    lockout: Duration,
    slots: Mutex<HashMap<IpAddr, Slot>>,
}

impl RateLimiter {
    pub fn new(max: u32, window: Duration, lockout: Duration) -> Self {
        Self {
            max,
            window,
            lockout,
            slots: Mutex::new(HashMap::new()),
        }
    }

    /// True if the IP may proceed. Records nothing.
    pub fn allowed(&self, ip: IpAddr) -> bool {
        if self.max == 0 {
            return true;
        }
        let now = Instant::now();
        lock(&self.slots)
            .get(&ip_key(ip))
            .is_none_or(|s| !self.blocked(s, now))
    }

    fn blocked(&self, s: &Slot, now: Instant) -> bool {
        if s.locked_until.is_some_and(|t| t > now) {
            return true;
        }
        self.lockout.is_zero() && s.count >= self.max && now.duration_since(s.start) < self.window
    }

    pub fn hit(&self, ip: IpAddr) {
        if self.max == 0 {
            return;
        }
        let now = Instant::now();
        let mut slots = lock(&self.slots);
        if slots.len() >= MAX_TRACKED {
            slots.retain(|_, s| self.live(s, now));
            // ponytail: still full means a distributed flood; forgetting everyone fails open
            // for it but keeps memory bounded. Per-subnet eviction if that ever matters.
            if slots.len() >= MAX_TRACKED {
                slots.clear();
            }
        }
        let s = slots.entry(ip_key(ip)).or_insert(Slot {
            start: now,
            count: 0,
            locked_until: None,
        });
        let expired = match s.locked_until {
            Some(t) => t <= now,
            None => now.duration_since(s.start) >= self.window,
        };
        if expired {
            *s = Slot {
                start: now,
                count: 0,
                locked_until: None,
            };
        }
        s.count += 1;
        if s.count >= self.max && !self.lockout.is_zero() && s.locked_until.is_none() {
            s.locked_until = Some(now + self.lockout);
        }
    }

    /// For limits that count attempts rather than failures.
    pub fn check_and_hit(&self, ip: IpAddr) -> bool {
        let ok = self.allowed(ip);
        if ok {
            self.hit(ip);
        }
        ok
    }

    fn live(&self, s: &Slot, now: Instant) -> bool {
        s.locked_until.is_some_and(|t| t > now) || now.duration_since(s.start) < self.window
    }

    /// Drops expired entries; called periodically.
    pub fn prune(&self) {
        let now = Instant::now();
        lock(&self.slots).retain(|_, s| self.live(s, now));
    }

    pub fn tracked(&self) -> usize {
        lock(&self.slots).len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn conn_cap_per_ip_and_release() {
        let t = ConnTracker::new(2);
        let a = t.acquire(ip("203.0.113.1")).unwrap();
        let _b = t.acquire(ip("203.0.113.1")).unwrap();
        assert!(t.acquire(ip("203.0.113.1")).is_none());
        assert!(t.acquire(ip("203.0.113.2")).is_some());
        drop(a);
        assert!(t.acquire(ip("203.0.113.1")).is_some());
        let unlimited = ConnTracker::new(0);
        let _g: Vec<_> = (0..100)
            .map(|_| unlimited.acquire(ip("1.1.1.1")).unwrap())
            .collect();
    }

    #[test]
    fn ipv6_is_keyed_by_slash_64() {
        assert_eq!(
            ip_key(ip("2001:db8::1:2:3:4")),
            ip_key(ip("2001:db8::ffff:ffff:ffff:ffff"))
        );
        assert_eq!(ip_key(ip("::ffff:1.2.3.4")), ip("1.2.3.4"));
    }

    #[test]
    fn lan_ranges() {
        for a in [
            "127.0.0.1",
            "::1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.1",
            "192.168.1.1",
            "fd12::1",
            "fe80::1",
            "169.254.1.1",
        ] {
            assert!(is_lan(ip(a)), "{a}");
        }
        for a in ["8.8.8.8", "172.32.0.1", "2001:db8::1", "100.64.0.1"] {
            assert!(!is_lan(ip(a)), "{a}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn failure_lockout_then_recovery() {
        let l = RateLimiter::new(3, Duration::from_secs(60), Duration::from_secs(30));
        let a = ip("203.0.113.1");
        for _ in 0..3 {
            assert!(l.allowed(a));
            l.hit(a);
        }
        assert!(!l.allowed(a));
        assert!(l.allowed(ip("203.0.113.2")));
        tokio::time::advance(Duration::from_secs(29)).await;
        assert!(!l.allowed(a));
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(l.allowed(a));
    }

    #[tokio::test(start_paused = true)]
    async fn windowed_limit_rolls_over_and_prunes() {
        let l = RateLimiter::new(2, Duration::from_secs(60), Duration::ZERO);
        let a = ip("203.0.113.1");
        assert!(l.check_and_hit(a));
        assert!(l.check_and_hit(a));
        assert!(!l.check_and_hit(a));
        tokio::time::advance(Duration::from_secs(61)).await;
        assert_eq!(l.tracked(), 1);
        l.prune();
        assert_eq!(l.tracked(), 0);
        assert!(l.check_and_hit(a));
        assert!(RateLimiter::new(0, Duration::from_secs(1), Duration::ZERO).check_and_hit(a));
    }

    #[tokio::test(start_paused = true)]
    async fn tracking_is_bounded() {
        let l = RateLimiter::new(5, Duration::from_secs(60), Duration::from_secs(60));
        for i in 0..(MAX_TRACKED as u32 + 50) {
            l.hit(IpAddr::from(i.to_be_bytes()));
        }
        assert!(l.tracked() <= MAX_TRACKED);
    }

    #[test]
    fn forwarded_for_only_with_trusted_proxy() {
        let proxy = ip("10.0.0.1");
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            "6.6.6.6, 203.0.113.9, 10.0.0.2".parse().unwrap(),
        );
        // Untrusted peer: header ignored.
        assert_eq!(
            client_ip(ip("198.51.100.1"), &h, &[proxy]),
            ip("198.51.100.1")
        );
        // Right-most untrusted entry wins; the spoofed left entry is ignored.
        assert_eq!(
            client_ip(proxy, &h, &[proxy, ip("10.0.0.2")]),
            ip("203.0.113.9")
        );
        let mut bad = HeaderMap::new();
        bad.insert("x-forwarded-for", "garbage".parse().unwrap());
        assert_eq!(client_ip(proxy, &bad, &[proxy]), proxy);
        assert_eq!(client_ip(proxy, &HeaderMap::new(), &[proxy]), proxy);
    }
}
