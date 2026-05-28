use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use sqlx::SqlitePool;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    OnceLock,
};

static SERVER_START: OnceLock<std::time::Instant> = OnceLock::new();
static HEALTH_DB_OK: AtomicBool = AtomicBool::new(true);
static HEALTH_LAST_CHECK_SECS: AtomicU64 = AtomicU64::new(0);

const HEALTH_CACHE_TTL_SECS: u64 = 10;

/// Call once at server startup to anchor the uptime clock.
pub fn init() {
    let _ = SERVER_START.set(std::time::Instant::now());
}

/// GET /health — unauthenticated. DB ping is cached for 10 s to prevent
/// connection-pool exhaustion from high-frequency polling.
pub async fn health(State(pool): State<SqlitePool>) -> impl IntoResponse {
    let uptime_secs = SERVER_START
        .get()
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last = HEALTH_LAST_CHECK_SECS.load(Ordering::Relaxed);

    if now.saturating_sub(last) >= HEALTH_CACHE_TTL_SECS {
        let db_ok = sqlx::query("SELECT 1").execute(&pool).await.is_ok();
        if !db_ok {
            tracing::error!("health check: db ping failed");
        }
        HEALTH_DB_OK.store(db_ok, Ordering::Relaxed);
        HEALTH_LAST_CHECK_SECS.store(now, Ordering::Relaxed);
    }

    if HEALTH_DB_OK.load(Ordering::Relaxed) {
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "ok",
                "db": "ok",
                "uptime_secs": uptime_secs,
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "degraded",
                "db": "error",
                "uptime_secs": uptime_secs,
            })),
        )
    }
}
