use std::collections::HashMap;
use std::hash::Hash;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Sliding-window rate limiter, generic over key type.
/// Use `RateLimiter<IpAddr>` for per-IP limits and `RateLimiter<String>` for per-mailbox limits.
#[derive(Clone)]
pub(crate) struct RateLimiter<K = IpAddr>
where
    K: Eq + Hash + Clone + Send + 'static,
{
    state: Arc<Mutex<HashMap<K, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

impl<K> RateLimiter<K>
where
    K: Eq + Hash + Clone + Send + 'static,
{
    pub(crate) fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    /// Returns `true` if the request is allowed, `false` if the limit is exceeded.
    pub(crate) fn check(&self, key: K) -> bool {
        let now = Instant::now();
        let mut map = self.state.lock().unwrap();
        let timestamps = map.entry(key).or_default();

        timestamps.retain(|t| now.duration_since(*t) < self.window);

        if timestamps.len() >= self.max_requests {
            return false;
        }

        timestamps.push(now);
        true
    }
}
