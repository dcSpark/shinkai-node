use async_lock::Mutex;
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::collections::HashMap;





use governor::clock::DefaultClock;
use governor::state::keyed::DefaultKeyedStateStore;

// Define a struct to hold your rate limiter and connection tracking.
pub struct ConnectionLimiter {
    pub rate_limiter: RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>,
    pub connections: Mutex<HashMap<String, usize>>,
    pub max_connections_per_ip: usize,
}

impl ConnectionLimiter {
    pub fn new(rate_per_second: u32, burst_size: u32, max_connections_per_ip: usize) -> Self {
        // Initialize the keyed rate limiter
        let rate_limiter = RateLimiter::keyed(Quota::per_second(NonZeroU32::new(rate_per_second).unwrap()).allow_burst(NonZeroU32::new(burst_size).unwrap()));
        let connections = Mutex::new(HashMap::new());
        ConnectionLimiter {
            rate_limiter,
            connections,
            max_connections_per_ip,
        }
    }

    pub async fn check_rate_limit(&self, ip: &str) -> bool {
        // Check the rate limit for a specific key (IP address)
        self.rate_limiter.check_key(&ip.to_string()).is_ok()
    }

    pub async fn increment_connection(&self, ip: &str) -> bool {
        let mut connections = self.connections.lock().await;
        let entry = connections.entry(ip.to_string()).or_insert(0);
        if *entry < self.max_connections_per_ip {
            *entry += 1;
            true
        } else {
            false
        }
    }

    pub async fn decrement_connection(&self, ip: &str) {
        let mut connections = self.connections.lock().await;
        if let Some(entry) = connections.get_mut(ip) {
            *entry -= 1;
            if *entry == 0 {
                connections.remove(ip);
            }
        }
    }
}