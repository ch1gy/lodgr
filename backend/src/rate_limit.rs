/// Per-IP token-bucket rate limiter implemented without external crates.
/// Each IP gets a bucket refilled at `rate_per_sec` tokens/second up to
/// `capacity`. One token is consumed per request. Returns 429 when empty.
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Instant,
};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct IpRateLimiter {
    state: Arc<Mutex<HashMap<IpAddr, Bucket>>>,
    rate_per_sec: f64,
    capacity: f64,
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

impl IpRateLimiter {
    pub fn new(per_second: u32, burst: u32) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            rate_per_sec: per_second as f64,
            capacity: burst as f64,
        }
    }

    async fn allow(&self, ip: IpAddr) -> bool {
        let mut map = self.state.lock().await;
        let now = Instant::now();
        let rate = self.rate_per_sec;
        let cap = self.capacity;

        let bucket = map.entry(ip).or_insert(Bucket {
            tokens: cap,
            last_refill: now,
        });

        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * rate).min(cap);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Axum `from_fn` middleware — apply with `.layer(Extension(limiter))` on the
/// routes you want to protect.
pub async fn rate_limit_by_ip(
    Extension(limiter): Extension<IpRateLimiter>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !limiter.allow(addr.ip()).await {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    next.run(req).await
}
