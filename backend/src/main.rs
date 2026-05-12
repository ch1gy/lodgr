mod auth;
mod config;
mod crypto;
mod db;
mod dto;
mod error;
mod middleware;
mod models;
mod notify;
mod rate_limit;
mod routes;
mod services;
mod ticket_status;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{DefaultBodyLimit, FromRef},
    http::{HeaderName, HeaderValue},
    middleware::from_fn,
    routing::{delete, get, patch, post},
    Extension, Router,
};
use sqlx::SqlitePool;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

use config::Config;
use crypto::EncryptionKey;
use rate_limit::{IpRateLimiter, rate_limit_by_ip};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    /// Derived AES-256-GCM key. Never logged or returned by any endpoint.
    pub enc_key: EncryptionKey,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(s: &AppState) -> Self { s.pool.clone() }
}

impl FromRef<AppState> for Config {
    fn from_ref(s: &AppState) -> Self { s.config.clone() }
}

impl FromRef<AppState> for EncryptionKey {
    fn from_ref(s: &AppState) -> Self { Arc::clone(&s.enc_key) }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("backend=info".parse()?),
        )
        .init();

    let config = Config::from_env()?;

    let enc_key: EncryptionKey = {
        let passphrase = std::env::var("ENCRYPTION_PASSPHRASE").unwrap_or_else(|_| {
            panic!("FATAL: ENCRYPTION_PASSPHRASE environment variable is not set")
        });
        let salt_hex = std::env::var("ENCRYPTION_SALT").unwrap_or_else(|_| {
            panic!("FATAL: ENCRYPTION_SALT environment variable is not set")
        });
        tracing::info!("Deriving encryption key — this may take a moment…");
        let raw = crypto::derive_key(&passphrase, &salt_hex)?;
        tracing::info!("Encryption key ready.");
        Arc::new(raw)
    };

    tokio::fs::create_dir_all("data").await?;
    tokio::fs::create_dir_all("uploads").await?;

    let pool = SqlitePool::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Startup session cleanup — also runs every 24 h below.
    match db::sessions::delete_expired_and_revoked(&pool).await {
        Ok(n) => tracing::info!("startup session cleanup: removed {n} expired/revoked rows"),
        Err(e) => tracing::warn!("startup session cleanup failed: {e}"),
    }

    seed_desk_user(&pool).await?;

    // Pre-warm the constant-time dummy hash used in login to avoid
    // argon2 overhead on the very first real login request.
    let _ = services::auth::dummy_hash_warmup();

    services::auth::check_default_password_warning(&pool).await;

    // ── Background session cleanup every 24 h ────────────────────────────────
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(24 * 3600));
        interval.tick().await; // skip first immediate tick (already ran above)
        loop {
            interval.tick().await;
            match db::sessions::delete_expired_and_revoked(&cleanup_pool).await {
                Ok(n) => tracing::info!("session cleanup: removed {n} expired/revoked rows"),
                Err(e) => tracing::warn!("session cleanup failed: {e}"),
            }
        }
    });

    // ── Rate limiter: 5 req/s per IP, burst of 10 — auth routes only ─────────
    let auth_limiter = IpRateLimiter::new(5, 10);

    let state = AppState { pool, config, enc_key };

    let app = Router::new()
        // Rate-limited auth routes (login + refresh only)
        .route("/auth/login",   post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route_layer(from_fn(rate_limit_by_ip))
        .layer(Extension(auth_limiter))
        // Logout has no rate limit
        .route("/auth/logout", post(auth::logout))
        // Admin
        .route("/admin/clients", post(routes::admin::create_client))
        .route(
            "/admin/clients/:id/sessions",
            delete(routes::admin::delete_client_sessions),
        )
        // Tickets
        .route(
            "/tickets",
            get(routes::tickets::list).post(routes::tickets::create),
        )
        .route("/tickets/:id",          get(routes::tickets::get))
        .route("/tickets/:id/ack",      patch(routes::tickets::ack))
        .route("/tickets/:id/close",    patch(routes::tickets::close))
        .route("/tickets/:id/message",  post(routes::messages::post_message))
        // Static frontend
        .nest_service("/", ServeDir::new("static"))
        // ── Global middleware (applied to every route) ────────────────────────
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MiB cap on every request body
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; object-src 'none'; frame-ancestors 'none'",
            ),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=63072000; includeSubDomains"),
        ))
        .with_state(state);

    let bind_addr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    println!("Listening on http://{bind_addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn seed_desk_user(pool: &SqlitePool) -> anyhow::Result<()> {
    if db::users::find_by_email(pool, "desk@local").await?.is_none() {
        let id = uuid::Uuid::new_v4().to_string();
        let hash = services::auth::hash_password("changeme")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        db::users::create(pool, &id, "Desk Agent", "desk@local", &hash, "desk").await?;
        println!("Seeded desk user: desk@local / changeme");
    }
    Ok(())
}
