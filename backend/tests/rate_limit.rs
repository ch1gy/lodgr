use backend::rate_limit::IpRateLimiter;
use std::net::IpAddr;

// ── IpRateLimiter ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn allow_returns_true_under_burst_limit() {
    let limiter = IpRateLimiter::new(10, 3); // 10 req/s, burst 3
    let ip: IpAddr = "127.0.0.1".parse().unwrap();
    assert!(limiter.allow(ip).await); // 1
    assert!(limiter.allow(ip).await); // 2
    assert!(limiter.allow(ip).await); // 3
}

#[tokio::test]
async fn allow_returns_false_when_burst_exceeded() {
    let limiter = IpRateLimiter::new(1, 2); // 1 req/s, burst 2
    let ip: IpAddr = "10.0.0.1".parse().unwrap();
    assert!(limiter.allow(ip).await); // 1 — ok
    assert!(limiter.allow(ip).await); // 2 — ok (burst)
    assert!(!limiter.allow(ip).await); // 3 — rejected
}

#[tokio::test]
async fn bucket_refills_over_time() {
    let limiter = IpRateLimiter::new(100, 1); // very fast rate so we don't need to sleep long
    let ip: IpAddr = "10.0.0.2".parse().unwrap();
    assert!(limiter.allow(ip).await); // consume the burst token
    assert!(!limiter.allow(ip).await); // empty

    // Sleep slightly more than 1/100 second (10 ms) — the 100 req/s rate
    // should refill at least one token.
    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    assert!(limiter.allow(ip).await);
}

#[tokio::test]
async fn different_ips_have_independent_buckets() {
    let limiter = IpRateLimiter::new(1, 1);
    let ip1: IpAddr = "192.168.1.1".parse().unwrap();
    let ip2: IpAddr = "192.168.1.2".parse().unwrap();

    assert!(limiter.allow(ip1).await); // ip1 — ok
    assert!(!limiter.allow(ip1).await); // ip1 — exhausted
    assert!(limiter.allow(ip2).await); // ip2 — still has its own token
}
