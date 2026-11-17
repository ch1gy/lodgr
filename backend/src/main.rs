// All modules live in lib.rs so integration tests can import them.
use backend::{auth, crypto, db, email, routes, services, tasks};
use backend::{
    config::{Config, Database},
    crypto::EncryptionKey,
    email::SmtpMailer,
    rate_limit::{rate_limit_by_ip, rate_limit_reports, IpRateLimiter, ReportRateLimiter},
};

use std::{future::IntoFuture, net::SocketAddr, sync::Arc};

use axum::{
    extract::{DefaultBodyLimit, FromRef},
    http::{HeaderName, HeaderValue},
    middleware::from_fn,
    routing::{delete, get, patch, post},
    Extension, Router,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub enc_key: EncryptionKey,
    pub mailer: Option<Arc<SmtpMailer>>,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(s: &AppState) -> Self {
        s.pool.clone()
    }
}
impl FromRef<AppState> for Config {
    fn from_ref(s: &AppState) -> Self {
        s.config.clone()
    }
}
impl FromRef<AppState> for EncryptionKey {
    fn from_ref(s: &AppState) -> Self {
        Arc::clone(&s.enc_key)
    }
}
impl FromRef<AppState> for Option<Arc<SmtpMailer>> {
    fn from_ref(s: &AppState) -> Self {
        s.mailer.clone()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env()?;

    // Everything the server writes (uploads/, exports/, logs/) is a relative
    // path, so pin the working directory to DATA_DIR before anything is
    // created. With DATA_DIR unset the legacy layout applies: the directory
    // the process was started from.
    if config.data_dir_explicit {
        std::fs::create_dir_all(&config.data_dir)?;
        std::env::set_current_dir(&config.data_dir)?;
    }

    // ── Logging: daily-rotating file + stdout ────────────────────────────────
    // Retention is by age (tasks::daily_maintenance deletes files older than
    // 30 days), not by file count: a count cap keeps old logs forever when the
    // server only runs now and then.
    std::fs::create_dir_all("logs")?;

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("lodgr")
        .filename_suffix("log")
        .build("logs")
        .map_err(|e| anyhow::anyhow!("log appender init: {e}"))?;

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::from_default_env().add_directive("backend=info".parse()?);

    // Single global filter applied to both stdout and file layers.
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false), // no ANSI escape codes in log files
        )
        .init();

    tracing::info!(data_dir = %config.data_dir.display(), "data directory");
    if !config.data_dir_explicit {
        tracing::warn!(
            "DATA_DIR is not set: data lives in the directory the server was started from ({}). Starting it from another folder would create a second, empty database. Set DATA_DIR to pin it.",
            config.data_dir.display()
        );
    }
    tracing::info!(static_dir = %config.static_dir.display(), "frontend directory");
    if !config.static_dir.join("index.html").is_file() {
        let msg = format!(
            "STATIC_DIR {} has no index.html — build the frontend (npm run build in frontend/) or set STATIC_DIR",
            config.static_dir.display()
        );
        // In production (COOKIE_SECURE=true) the backend serves the app, so a
        // missing build is fatal. In development Vite serves it instead.
        if config.cookie_secure {
            anyhow::bail!(msg);
        }
        tracing::warn!("{msg}");
    }

    let enc_key: EncryptionKey = {
        let passphrase = zeroize::Zeroizing::new(
            std::env::var("ENCRYPTION_PASSPHRASE")
                .unwrap_or_else(|_| panic!("FATAL: ENCRYPTION_PASSPHRASE not set")),
        );
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

    // Create the DB parent directory, and the upload/export dirs (relative to
    // the data directory, which is the working directory by now).
    let connect_opts = match &config.database {
        Database::File(path) => {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            if !path.exists() {
                tracing::warn!(
                    "database file {} does not exist — creating a NEW, empty database (and a new desk account). If you expected existing data, stop now and check DATA_DIR / the folder you started the server from.",
                    path.display()
                );
            }
            tracing::info!(database = %path.display(), "database file");
            SqliteConnectOptions::new().filename(path)
        }
        Database::Url(url) => {
            let file = url
                .strip_prefix("sqlite:///")
                .or_else(|| url.strip_prefix("sqlite://"));
            if let Some(parent) = file.and_then(|f| std::path::Path::new(f).parent()) {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent).await?;
                }
            }
            tracing::info!("database: explicit DATABASE_URL");
            SqliteConnectOptions::from_str(url)?
        }
    };
    for dir in &["uploads", "exports"] {
        tokio::fs::create_dir_all(dir).await?;
    }

    let connect_opts = connect_opts
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        // Retry for up to 5 s when a concurrent write holds the lock.
        // Without this, simultaneous writes return SQLITE_BUSY immediately.
        .busy_timeout(std::time::Duration::from_millis(5_000));
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(connect_opts)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Startup cleanup.
    if let Ok(n) = db::sessions::delete_expired_and_revoked(&pool).await {
        tracing::info!("startup: removed {n} expired/revoked sessions");
    }
    if let Ok(n) = db::jwt_revocations::delete_expired(&pool).await {
        if n > 0 {
            tracing::info!("startup: removed {n} expired jti rows");
        }
    }

    seed_desk_user(&pool, &enc_key, &config.desk_email, &config.email_hash_salt).await?;
    services::auth::dummy_hash_warmup();
    services::auth::check_default_password_warning(
        &pool,
        &config.desk_email,
        &config.email_hash_salt,
    )
    .await;
    check_desk_lockout(&pool, &config.desk_email, &config.email_hash_salt).await;

    // Background tasks — wrapped in a restart supervisor so panics are logged
    // and the task is relaunched after a 5-second cooldown.
    routes::health::init();
    spawn_with_restart("recurring_tickets", pool.clone(), tasks::recurring_tickets);
    spawn_with_restart(
        "recurring_invoices",
        pool.clone(),
        tasks::recurring_invoices,
    );
    {
        // Clone the key before it moves into AppState so the task can own a copy.
        let enc_key_for_task = Arc::clone(&enc_key);
        spawn_with_restart("hard_delete_expired_users", pool.clone(), move |pool| {
            let key = Arc::clone(&enc_key_for_task);
            tasks::hard_delete_expired_users(pool, key)
        });
    }

    // Housekeeping that must also run on short uptimes: a first pass about a
    // minute after start, then on the usual interval (see tasks::STARTUP_DELAY).
    spawn_with_restart("daily_maintenance", pool.clone(), tasks::daily_maintenance);
    spawn_with_restart("export_cleanup", pool.clone(), |_pool| {
        tasks::export_cleanup()
    });

    let auth_limiter = IpRateLimiter::new(config.rate_limit_auth_rps, config.rate_limit_auth_burst);
    let report_limiter = ReportRateLimiter(IpRateLimiter::with_rate(
        config.rate_limit_report_rps,
        config.rate_limit_report_burst,
    ));
    let shutdown_pool = pool.clone();
    let state = AppState {
        pool,
        config,
        enc_key,
        mailer,
    };

    let static_dir = state.config.static_dir.clone();

    // ── Rate-limited sub-routers ───────────────────────────────────────────
    // route_layer() in Axum applies to ALL routes currently in the router, so
    // each limiter lives in its own sub-router and is merged in below. This
    // prevents the report limiter (0.2 req/s) from leaking onto ticket/admin routes.
    let auth_limited = Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/magic", post(routes::magic::exchange))
        .route("/auth/magic-request", post(routes::magic::magic_request))
        .route_layer(from_fn(rate_limit_by_ip))
        .layer(Extension(auth_limiter));

    let report_limited = Router::new()
        .route("/reports/range/:client_id", get(routes::reports::range))
        .route_layer(from_fn(rate_limit_reports))
        .layer(Extension(report_limiter));

    let app = Router::new()
        // ── Health — unauthenticated ───────────────────────────────────────
        .route("/health", get(routes::health::health))
        // ── Auth (rate-limited) ────────────────────────────────────────────
        .merge(auth_limited)
        // ── Auth — no rate limit ───────────────────────────────────────────
        .route("/auth/logout", post(auth::logout))
        .route("/auth/password", patch(auth::change_password))
        .route("/auth/me", get(auth::me))
        .route("/auth/profile", patch(auth::update_my_profile))
        .route("/auth/sessions", get(auth::list_sessions))
        .route("/auth/sessions/:id", delete(auth::revoke_session))
        // ── Admin ──────────────────────────────────────────────────────────
        .route("/admin/clients", post(routes::admin::create_client))
        .route("/admin/clients", get(routes::admin::list_clients))
        .route(
            "/admin/clients/:id/sessions",
            delete(routes::admin::delete_client_sessions),
        )
        .route(
            "/admin/clients/:id/soft-delete",
            post(routes::admin::soft_delete_client),
        )
        .route(
            "/admin/clients/:id/restore",
            post(routes::admin::restore_client),
        )
        .route(
            "/admin/clients/:id/unlock",
            post(routes::admin::unlock_client),
        )
        .route(
            "/admin/clients/:id",
            patch(routes::admin::update_client).delete(routes::admin::hard_delete_client),
        )
        .route(
            "/admin/clients/:id/export",
            post(routes::admin::export_client),
        )
        .route(
            "/admin/clients/:id/magic-link",
            post(routes::admin::create_full_magic_link),
        )
        .route(
            "/admin/clients/:id/auth-events",
            get(routes::admin::get_auth_events),
        )
        .route(
            "/admin/clients/:id/sub-clients",
            get(routes::admin::list_sub_clients).post(routes::admin::create_sub_client),
        )
        .route(
            "/admin/sub-clients/:sub_id",
            delete(routes::admin::delete_sub_client),
        )
        // ── Desk profile ───────────────────────────────────────────────────
        .route(
            "/admin/desk-profile",
            get(routes::admin::get_desk_profile).put(routes::admin::update_desk_profile),
        )
        // ── Invoices ───────────────────────────────────────────────────────
        .route(
            "/admin/invoices",
            get(routes::invoices::list).post(routes::invoices::create),
        )
        .route(
            "/admin/invoices/:id",
            get(routes::invoices::get)
                .patch(routes::invoices::update)
                .delete(routes::invoices::delete),
        )
        .route(
            "/admin/invoices/:id/print",
            get(routes::invoices::print_html),
        )
        .route(
            "/admin/invoices/:id/pdf",
            get(routes::invoices::pdf_download),
        )
        .route(
            "/admin/exports/:client_id/:filename",
            get(routes::admin::get_export_file),
        )
        // ── Tickets ────────────────────────────────────────────────────────
        .route(
            "/tickets",
            get(routes::tickets::list).post(routes::tickets::create),
        )
        .route(
            "/tickets/:id",
            get(routes::tickets::get)
                .patch(routes::tickets::update)
                .delete(routes::tickets::delete),
        )
        .route("/tickets/:id/ack", patch(routes::tickets::ack))
        .route("/tickets/:id/pend", patch(routes::tickets::pend))
        .route("/tickets/:id/close", patch(routes::tickets::close))
        .route("/tickets/:id/reopen", patch(routes::tickets::reopen))
        .route(
            "/tickets/:id/message",
            post(routes::messages::post_message).layer(DefaultBodyLimit::max(100 * 1024 * 1024)),
        )
        .route(
            "/tickets/:id/notes",
            get(routes::notes::list).post(routes::notes::create),
        )
        .route(
            "/tickets/:id/magic-link",
            post(routes::magic::create_ticket_scoped),
        )
        // ── Uploads — auth-gated file downloads ───────────────────────────
        .route(
            "/uploads/:ticket_id/:filename",
            get(routes::messages::get_attachment),
        )
        // ── Reports (rate-limited) ─────────────────────────────────────────
        .merge(report_limited)
        // ── Static frontend ────────────────────────────────────────────────
        // In dev, Vite handles this via its proxy. In production (cargo run
        // from the backend/ dir), serve the built React app with an SPA
        // fallback so React Router can handle client-side paths like /clients.
        .nest_service(
            "/",
            ServeDir::new(&static_dir).fallback(ServeFile::new(static_dir.join("index.html"))),
        )
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
        // Outermost: every log line emitted while handling a request, including
        // AppError::Internal, carries the method and path. Path only: the query
        // string is never logged.
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
                tracing::info_span!("request", method = %req.method(), path = %req.uri().path())
            }),
        )
        .with_state(state);

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("Listening on http://{bind_addr}");

    // Stop on Ctrl-C / SIGTERM / console close, let in-flight requests finish
    // (bounded: a stuck connection must not make the server unkillable), then
    // fold the write-ahead log into the main database file and close the pool.
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            stop_rx.await.ok();
        })
        .into_future(),
    );
    tokio::pin!(server);

    tokio::select! {
        res = &mut server => {
            // The server stopped by itself: surface why.
            res??;
        }
        () = shutdown_signal() => {
            tracing::info!("shutdown requested — finishing in-flight requests");
            stop_tx.send(()).ok();
            match tokio::time::timeout(SHUTDOWN_GRACE, &mut server).await {
                Ok(res) => res??,
                Err(_) => tracing::warn!("in-flight requests did not finish in time — closing anyway"),
            }
        }
    }

    if let Err(e) = db::checkpoint_wal(&shutdown_pool).await {
        tracing::warn!("WAL checkpoint at shutdown failed: {e}");
    }
    shutdown_pool.close().await;
    tracing::info!("shutdown complete");

    Ok(())
}

/// How long in-flight requests get to finish after a shutdown signal.
const SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_secs(10);

/// Resolves on Ctrl-C, SIGTERM (Unix), or Ctrl-Break / the console window being
/// closed / the machine shutting down (Windows).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(windows)]
    let terminate = async {
        let close = tokio::signal::windows::ctrl_close();
        let shutdown = tokio::signal::windows::ctrl_shutdown();
        let brk = tokio::signal::windows::ctrl_break();
        match (close, shutdown, brk) {
            (Ok(mut c), Ok(mut s), Ok(mut b)) => {
                tokio::select! {
                    _ = c.recv() => {}
                    _ = s.recv() => {}
                    _ = b.recv() => {}
                }
            }
            _ => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(any(unix, windows)))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

/// Spawns a background task inside a restart supervisor with exponential backoff.
/// A task that runs for ≥30 s before exiting resets the backoff counter.
/// Rapid consecutive failures back off: 5 s → 10 s → 30 s → 60 s → 5 min (cap).
fn spawn_with_restart<F, Fut>(name: &'static str, pool: SqlitePool, make_fut: F)
where
    F: Fn(SqlitePool) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        const BACKOFF: &[u64] = &[5, 10, 30, 60, 300];
        let mut fail_count: usize = 0;

        loop {
            let start = tokio::time::Instant::now();
            match tokio::spawn(make_fut(pool.clone())).await {
                Ok(()) => tracing::warn!(task = name, "task exited unexpectedly"),
                Err(e) => tracing::error!(task = name, err = %e, "task panicked"),
            }

            if start.elapsed().as_secs() >= 30 {
                fail_count = 0;
            } else {
                fail_count = fail_count.saturating_add(1);
            }

            let delay = BACKOFF[fail_count.saturating_sub(1).min(BACKOFF.len() - 1)];
            tracing::info!(task = name, delay_secs = delay, "restarting task");
            tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
        }
    });
}

async fn check_desk_lockout(pool: &SqlitePool, desk_email: &str, email_hash_salt: &str) {
    let hash = match crypto::hash_email(desk_email, email_hash_salt) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("check_desk_lockout: email hash failed: {e:?}");
            return;
        }
    };
    if let Ok(Some(user)) = db::users::find_by_email_hash(pool, &hash).await {
        if user.locked_until.is_some() {
            if user.failed_attempts >= services::auth::PERMANENT_LOCKOUT_THRESHOLD {
                tracing::error!(
                    failed_attempts = user.failed_attempts,
                    email = %desk_email,
                    "SECURITY: desk account is PERMANENTLY LOCKED. \
                     Recovery: UPDATE users SET failed_attempts=0, locked_until=NULL \
                     WHERE email='{desk_email}'"
                );
                eprintln!(
                    "\n╔══════════════════════════════════════════════════════════════╗\
                     \n║  SECURITY: desk account is PERMANENTLY LOCKED                ║\
                     \n║  Recovery: UPDATE users SET failed_attempts=0,               ║\
                     \n║    locked_until=NULL WHERE email='{desk_email}'              ║\
                     \n╚══════════════════════════════════════════════════════════════╝\n"
                );
            } else {
                tracing::warn!(
                    failed_attempts = user.failed_attempts,
                    locked_until = ?user.locked_until,
                    email = %desk_email,
                    "desk account is temporarily locked"
                );
            }
        }
    }
}

async fn seed_desk_user(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    desk_email: &str,
    email_hash_salt: &str,
) -> anyhow::Result<()> {
    let email_hash = crypto::hash_email(desk_email, email_hash_salt)
        .map_err(|e| anyhow::anyhow!("email hash for seed: {e:?}"))?;

    if db::users::find_by_email_hash(pool, &email_hash)
        .await?
        .is_none()
    {
        let initial_password =
            std::env::var("DESK_INITIAL_PASSWORD").unwrap_or_else(|_| "changeme".to_string());
        let id = uuid::Uuid::new_v4().to_string();
        let hash = services::auth::hash_password(&initial_password)
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let (email_nonce, email_ct) = crypto::encrypt(enc_key, desk_email)
            .map_err(|e| anyhow::anyhow!("encrypt desk email: {e:?}"))?;
        db::users::create(
            pool,
            db::users::NewUser {
                id: &id,
                name: "Desk Agent",
                email: &email_ct,
                email_nonce: &email_nonce,
                email_hash: &email_hash,
                password_hash: &hash,
                role: "desk",
            },
        )
        .await?;
        if initial_password == "changeme" {
            tracing::warn!(
                email = %desk_email,
                "Seeded desk account with the default password 'changeme'. \
                 Set DESK_INITIAL_PASSWORD in your environment to use a stronger one."
            );
        } else {
            tracing::info!(email = %desk_email, "Seeded desk account from DESK_INITIAL_PASSWORD.");
        }
    }
    Ok(())
}
