use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Allows at most `limit` sends per endpoint in each 1-second window.
pub struct RateLimiter {
    windows: Mutex<HashMap<i64, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true if this endpoint may send now, and counts the send.
    pub fn try_acquire(&self, endpoint_id: i64, limit: u32) -> bool {
        self.try_acquire_at(endpoint_id, limit, Instant::now())
    }

    fn try_acquire_at(&self, endpoint_id: i64, limit: u32, now: Instant) -> bool {
        let mut windows = self.windows.lock().expect("rate limiter lock poisoned");
        let entry = windows.entry(endpoint_id).or_insert((now, 0));

        // A new second has started: reset the count.
        if now.duration_since(entry.0) >= Duration::from_secs(1) {
            *entry = (now, 0);
        }

        if entry.1 < limit {
            entry.1 += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_the_limit() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        assert!(rl.try_acquire_at(1, 2, now));
        assert!(rl.try_acquire_at(1, 2, now));
    }

    #[test]
    fn blocks_after_the_limit() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        rl.try_acquire_at(1, 2, now);
        rl.try_acquire_at(1, 2, now);
        assert!(!rl.try_acquire_at(1, 2, now));
    }

    #[test]
    fn resets_after_one_second() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        rl.try_acquire_at(1, 1, now);
        assert!(rl.try_acquire_at(1, 1, now + Duration::from_secs(1)));
    }

    #[test]
    fn endpoints_have_separate_limits() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        rl.try_acquire_at(1, 1, now);
        assert!(rl.try_acquire_at(2, 1, now));
    }
}