use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Sliding-window per-IP rate limiter.
/// Tracks request timestamps per IP and rejects if more than `max_requests`
/// have occurred within `window`.
#[derive(Clone)]
pub(crate) struct RateLimiter {
    state: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub(crate) fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    /// Returns `true` if the request is allowed, `false` if rate limit exceeded.
    pub(crate) fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut map = self.state.lock().unwrap();
        let timestamps = map.entry(ip).or_default();

        // Drop timestamps outside the window.
        timestamps.retain(|t| now.duration_since(*t) < self.window);

        if timestamps.len() >= self.max_requests {
            return false;
        }

        timestamps.push(now);
        true
    }
}
