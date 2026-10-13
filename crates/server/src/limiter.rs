//! Per-connection token bucket. Uses tokio's clock so tests can pause time.

use tokio::time::Instant;

pub struct TokenBucket {
    capacity: f64,
    refill_per_sec: f64,
    tokens: f64,
    last: Instant,
}

impl TokenBucket {
    pub fn new(refill_per_sec: f64, capacity: f64) -> Self {
        Self {
            capacity,
            refill_per_sec,
            tokens: capacity,
            last: Instant::now(),
        }
    }

    /// Takes one token; false means the caller is over its budget.
    pub fn try_take(&mut self) -> bool {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + dt * self.refill_per_sec).min(self.capacity);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn burst_then_refill() {
        let mut b = TokenBucket::new(10.0, 5.0);
        for _ in 0..5 {
            assert!(b.try_take());
        }
        assert!(!b.try_take());
        tokio::time::advance(Duration::from_millis(250)).await; // 2.5 tokens
        assert!(b.try_take());
        assert!(b.try_take());
        assert!(!b.try_take());
    }

    #[tokio::test(start_paused = true)]
    async fn refill_caps_at_capacity() {
        let mut b = TokenBucket::new(100.0, 3.0);
        tokio::time::advance(Duration::from_secs(60)).await;
        for _ in 0..3 {
            assert!(b.try_take());
        }
        assert!(!b.try_take());
    }
}
