mod auth;
mod config;
mod crypto;
mod db;
mod dto;
mod email;
mod error;
mod middleware;
mod models;
mod notify;
mod rate_limit;
mod routes;
mod services;
mod tasks;
mod ticket_status;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{DefaultBodyLimit, FromRef},
    http::{HeaderName, HeaderValue},
    middleware::from_fn,
    routing::{delete, get, patch, post},
    Extension, Router,
};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use config::Config;
use crypto::EncryptionKey;
use email::SmtpMailer;
use rate_limit::{rate_limit_by_ip, rate_limit_reports, IpRateLimiter, ReportRateLimiter};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub enc_key: EncryptionKey,
    pub mailer: Option<Arc<SmtpMailer>>,
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
impl FromRef<AppState> for Option<Arc<SmtpMailer>> {
    fn from_ref(s: &AppState) -> Self { s.mailer.clone() }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Logging: daily-rotating file (30-day retention) + stdout ─────────────
    std::fs::create_dir_all("logs")?;

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("lodgr")
        .filename_suffix("log")
        .max_log_files(30)
        .build("logs")
        .map_err(|e| anyhow::anyhow!("log appender init: {e}"))?;

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::from_default_env()
        .add_directive("backend=info".parse()?);

    // Single global filter applied to both stdout and file layers.
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),  // no ANSI escape codes in log files
        )
        .init();

    let config = Config::from_env()?;

    let enc_key: EncryptionKey = {
        let passphrase = std::env::var("ENCRYPTION_PASSPHRASE")
            .unwrap_or_else(|_| panic!("FATAL: ENCRYPTION_PASSPHRASE not set"));
        let salt_hex = std::env::var("ENCRYPTION_SALT")
            .unwrap_or_else(|_| panic!("FATAL: ENCRYPTION_SALT not set"));
        tracing::info!("Deriving encryption key — this may take a moment…");
        let raw = crypto::derive_key(&passphrase, &salt_hex)?;
        tracing::info!("Encryption key ready.");
        Arc::new(raw)
    };

    let mailer: Option<Arc<SmtpMailer>> = match email::SmtpMailer::from_config(&config) {
        Some(Ok(m)) => {
            tracing::info!("SMTP mailer configured.");
            Some(Arc::new(m))
        }
        Some(Err(e)) => {
            tracing::warn!("SMTP configured but failed to init: {e} — email disabled.");
            None
        }
        None => {
            tracing::info!("SMTP_HOST not set — email notifications disabled.");
            None
        }
    };

    // Create the DB parent directory from the configured URL.
    // Also create upload/export dirs relative to the process working dir.
    if let Some(db_path) = config.database_url.strip_prefix("sqlite:///") {
        if let Some(parent) = std::path::Path::new(db_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
    } else if let Some(db_path) = config.database_url.strip_prefix("sqlite://") {
        if let Some(parent) = std::path::Path::new(db_path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
    }
    for dir in &["uploads", "exports"] {
        tokio::fs::create_dir_all(dir).await?;
    }

    let connect_opts = SqliteConnectOptions::from_str(&config.database_url)?
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(connect_opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Startup cleanup.
    if let Ok(n) = db::sessions::delete_expired_and_revoked(&pool).await {
        tracing::info!("startup: removed {n} expired/revoked sessions");
    }

    seed_desk_user(&pool).await?;
    let _ = services::auth::dummy_hash_warmup();
    services::auth::check_default_password_warning(&pool).await;
    check_desk_lockout(&pool).await;

    // Background tasks.
    tokio::spawn(tasks::hard_delete_expired_users(pool.clone()));
    tokio::spawn(tasks::recurring_tickets(pool.clone()));

    {
        let cleanup_pool = pool.clone();
        tokio::spawn(async move {
            let mut iv = tokio::time::interval(tokio::time::Duration::from_secs(24 * 3600));
            iv.tick().await;
            loop {
                iv.tick().await;
                if let Ok(n) = db::sessions::delete_expired_and_revoked(&cleanup_pool).await {
                    tracing::info!("session cleanup: removed {n} rows");
                }
            }
        });
    }

    let auth_limiter = IpRateLimiter::new(5, 10);
    // PDF generation is CPU-intensive: 1 req/5 s per IP, burst of 3.
    let report_limiter = ReportRateLimiter(IpRateLimiter::with_rate(0.2, 3));
    let state = AppState { pool, config, enc_key, mailer };

    let app = Router::new()
        // ── Rate-limited auth endpoints ────────────────────────────────────
        .route("/auth/login",   post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/magic",   post(routes::magic::exchange))
        .route_layer(from_fn(rate_limit_by_ip))
        .layer(Extension(auth_limiter))
        // ── Auth — no rate limit ───────────────────────────────────────────
        .route("/auth/logout",   post(auth::logout))
        .route("/auth/password", patch(auth::change_password))
        // ── Admin ──────────────────────────────────────────────────────────
        .route("/admin/clients",                          post(routes::admin::create_client))
        .route("/admin/clients",                          get(routes::admin::list_clients))
        .route("/admin/clients/:id/sessions",             delete(routes::admin::delete_client_sessions))
        .route("/admin/clients/:id/soft-delete",          post(routes::admin::soft_delete_client))
        .route("/admin/clients/:id/restore",              post(routes::admin::restore_client))
        .route("/admin/clients/:id/unlock",               post(routes::admin::unlock_client))
        .route("/admin/clients/:id",                      delete(routes::admin::hard_delete_client))
        .route("/admin/clients/:id/export",               post(routes::admin::export_client))
        .route("/admin/clients/:id/magic-link",           post(routes::admin::create_full_magic_link))
        .route("/admin/exports/:client_id/:filename",     get(routes::admin::get_export_file))
        // ── Tickets ────────────────────────────────────────────────────────
        .route("/tickets",           get(routes::tickets::list).post(routes::tickets::create))
        .route("/tickets/:id",       get(routes::tickets::get).patch(routes::tickets::update))
        .route("/tickets/:id/ack",   patch(routes::tickets::ack))
        .route("/tickets/:id/pend",  patch(routes::tickets::pend))
        .route("/tickets/:id/close", patch(routes::tickets::close))
        .route("/tickets/:id/message",    post(routes::messages::post_message))
        .route("/tickets/:id/notes",      get(routes::notes::list).post(routes::notes::create))
        .route("/tickets/:id/magic-link", post(routes::magic::create_ticket_scoped))
        // ── Reports — separately rate-limited ─────────────────────────────
        .route("/reports/monthly/:client_id/:year/:month", get(routes::reports::monthly))
        .route_layer(from_fn(rate_limit_reports))
        .layer(Extension(report_limiter))
        // ── Static frontend ────────────────────────────────────────────────
        .nest_service("/", ServeDir::new("static"))
        // ── Global middleware ──────────────────────────────────────────────
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
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
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .with_state(state);

    let bind_addr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("Listening on http://{bind_addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn check_desk_lockout(pool: &SqlitePool) {
    if let Ok(Some(user)) = db::users::find_by_email(pool, "desk@local").await {
        if user.locked_until.is_some() {
            if user.failed_attempts >= 9 {
                tracing::error!(
                    failed_attempts = user.failed_attempts,
                    "SECURITY: desk@local is PERMANENTLY LOCKED. \
                     Recovery: sqlite3 data/support.db \
                     \"UPDATE users SET failed_attempts=0, locked_until=NULL \
                     WHERE email='desk@local'\""
                );
                eprintln!(
                    "\n╔══════════════════════════════════════════════════════════════╗\
                     \n║  SECURITY: desk@local is PERMANENTLY LOCKED                  ║\
                     \n║  Run this to recover:                                        ║\
                     \n║    sqlite3 data/support.db                                   ║\
                     \n║    \"UPDATE users SET failed_attempts=0, locked_until=NULL    ║\
                     \n║     WHERE email='desk@local'\"                               ║\
                     \n╚══════════════════════════════════════════════════════════════╝\n"
                );
            } else {
                tracing::warn!(
                    failed_attempts = user.failed_attempts,
                    locked_until = ?user.locked_until,
                    "desk@local is temporarily locked"
                );
            }
        }
    }
}

async fn seed_desk_user(pool: &SqlitePool) -> anyhow::Result<()> {
    if db::users::find_by_email(pool, "desk@local").await?.is_none() {
        let id = uuid::Uuid::new_v4().to_string();
        let hash = services::auth::hash_password("changeme")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        db::users::create(pool, &id, "Desk Agent", "desk@local", &hash, "desk").await?;
        tracing::info!("Seeded desk user: desk@local / changeme");
    }
    Ok(())
}
