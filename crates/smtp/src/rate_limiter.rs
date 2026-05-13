//! Per-IP and global connection rate limiter.
//!
//! `RateLimiter::try_acquire` returns a `ConnectionPermit` on success.
//! Dropping the permit automatically decrements both counters (RAII).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

pub(crate) struct RateLimiter {
    per_ip: Arc<Mutex<HashMap<IpAddr, usize>>>,
    total: Arc<Mutex<usize>>,
    max_per_ip: usize,
    max_total: usize,
}

pub(crate) struct ConnectionPermit {
    ip: IpAddr,
    per_ip: Arc<Mutex<HashMap<IpAddr, usize>>>,
    total: Arc<Mutex<usize>>,
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        let mut map = self.per_ip.lock().unwrap();
        if let Some(c) = map.get_mut(&self.ip) {
            *c -= 1;
            if *c == 0 {
                map.remove(&self.ip);
            }
        }
        *self.total.lock().unwrap() -= 1;
    }
}

impl RateLimiter {
    pub(crate) fn new(max_per_ip: usize, max_total: usize) -> Self {
        Self {
            per_ip: Arc::new(Mutex::new(HashMap::new())),
            total: Arc::new(Mutex::new(0)),
            max_per_ip,
            max_total,
        }
    }

    /// Returns `Some(permit)` if both limits allow the connection, `None` otherwise.
    pub(crate) fn try_acquire(&self, ip: IpAddr) -> Option<ConnectionPermit> {
        let mut total = self.total.lock().unwrap();
        if *total >= self.max_total {
            return None;
        }
        let mut map = self.per_ip.lock().unwrap();
        let count = map.entry(ip).or_insert(0);
        if *count >= self.max_per_ip {
            return None;
        }
        *count += 1;
        *total += 1;
        Some(ConnectionPermit {
            ip,
            per_ip: Arc::clone(&self.per_ip),
            total: Arc::clone(&self.total),
        })
    }
}
