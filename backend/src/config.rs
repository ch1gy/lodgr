use std::path::{Path, PathBuf};

use jsonwebtoken::{DecodingKey, EncodingKey};
use zeroize::Zeroizing;

/// Where the SQLite database lives.
#[derive(Clone, Debug)]
pub enum Database {
    /// A database file chosen by Lodgr itself (from `DATA_DIR`, or the legacy
    /// working-directory layout). Opened by path, so spaces and other URL
    /// metacharacters in the path are harmless.
    File(PathBuf),
    /// An explicit `DATABASE_URL`.
    Url(String),
}

/// Absolute locations resolved once at startup. See `resolve_paths`.
#[derive(Clone, Debug)]
pub struct Paths {
    /// Working directory the process runs in. `uploads/`, `exports/` and
    /// `logs/` are created beneath it and every relative path in the code
    /// resolves against it.
    pub data_dir: PathBuf,
    /// True when `DATA_DIR` was set; false for the legacy layout where
    /// `data_dir` is simply the directory the process was started from.
    pub data_dir_explicit: bool,
    pub database: Database,
    /// Built React app served at `/`.
    pub static_dir: PathBuf,
}

/// Joins `p` onto `base` when relative, and resolves `.` / `..` lexically so
/// the logged path is readable. (No filesystem access, so it works for
/// directories that do not exist yet.)
fn absolute(base: &Path, p: &str) -> PathBuf {
    let p = Path::new(p);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    };
    normalize(&joined)
}

fn normalize(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Decide where data and the built frontend live, independent of where the
/// server happens to be started from.
///
/// * `DATA_DIR` set: the database is `DATA_DIR/support.db` and `uploads/`,
///   `exports/` and `logs/` live inside it.
/// * `DATA_DIR` unset (legacy): everything is relative to the starting
///   directory (`data/support.db`, `uploads/`, `exports/`, `logs/`), exactly as
///   before, but now resolved to absolute paths so they can be logged.
/// * `DATABASE_URL`, when set, always wins for the database.
/// * `STATIC_DIR`, when set, always wins for the frontend. Otherwise the first
///   of `./frontend/dist`, `../frontend/dist` and the checkout the binary was
///   built in that contains an `index.html`.
pub fn resolve_paths(
    cwd: &Path,
    exe_dir: Option<&Path>,
    data_dir: Option<&str>,
    database_url: Option<&str>,
    static_dir: Option<&str>,
) -> Paths {
    let data_dir_set = data_dir.map(str::trim).filter(|s| !s.is_empty());
    let resolved_data_dir = data_dir_set.map_or_else(|| cwd.to_path_buf(), |d| absolute(cwd, d));

    let database = match database_url.map(str::trim).filter(|s| !s.is_empty()) {
        Some(url) => Database::Url(url.to_owned()),
        None if data_dir_set.is_some() => Database::File(resolved_data_dir.join("support.db")),
        None => Database::File(cwd.join("data").join("support.db")),
    };

    let resolved_static = match static_dir.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => absolute(cwd, s),
        None => {
            let mut candidates = vec![
                cwd.join("frontend").join("dist"),
                cwd.join("..").join("frontend").join("dist"),
            ];
            if let Some(exe) = exe_dir {
                // target/debug/backend -> <repo>/frontend/dist
                candidates.push(exe.join("..").join("..").join("frontend").join("dist"));
            }
            let found = candidates
                .iter()
                .find(|c| c.join("index.html").is_file())
                .cloned();
            normalize(&found.unwrap_or_else(|| cwd.join("..").join("frontend").join("dist")))
        }
    };

    Paths {
        data_dir: resolved_data_dir,
        data_dir_explicit: data_dir_set.is_some(),
        database,
        static_dir: resolved_static,
    }
}

/// How to connect to the SMTP server.
#[derive(Clone, Debug, Default)]
pub enum SmtpTls {
    #[default]
    Starttls,
    None,
}

#[derive(Clone)]
pub struct Config {
    pub database: Database,
    pub data_dir: PathBuf,
    pub data_dir_explicit: bool,
    pub static_dir: PathBuf,
    /// Whether the background job may permanently delete clients archived more
    /// than 30 days ago. Off unless `AUTO_PURGE_ARCHIVED_CLIENTS=true`: deleting
    /// client data automatically must be a deliberate choice.
    pub auto_purge_archived_clients: bool,
    pub jwt_secret: Zeroizing<String>,
    pub access_token_ttl_secs: i64,
    pub refresh_token_ttl_secs: i64,
    pub cookie_secure: bool,
    pub bind_addr: String,
    /// Public base URL used when building magic-link URLs. No trailing slash.
    /// Example: https://support.example.com
    pub base_url: String,
    pub scoped_token_ttl_secs: i64,
    pub magic_link_ttl_secs: i64,
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_password: Option<Zeroizing<String>>,
    pub smtp_from: Option<String>,
    pub smtp_tls: SmtpTls,
    /// Requests per second allowed per IP on auth endpoints (login, refresh, magic exchange).
    pub rate_limit_auth_rps: u32,
    /// Burst capacity for the auth rate limiter.
    pub rate_limit_auth_burst: u32,
    /// Requests per second allowed per IP on CPU-intensive report endpoints (fractional OK).
    pub rate_limit_report_rps: f64,
    /// Burst capacity for the report rate limiter.
    pub rate_limit_report_burst: u32,
    /// Email address for the built-in desk account.
    /// Defaults to "desk@local". Change this so the address isn't predictable
    /// from reading the open-source code — prevents targeted lockout attacks.
    pub desk_email: String,
    /// Fixed hex-encoded Argon2id salt for email blind-index hashing.
    /// Loaded from EMAIL_HASH_SALT. Must never change after first use.
    pub email_hash_salt: Zeroizing<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let cwd = std::env::current_dir()?;
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf));
        let paths = resolve_paths(
            &cwd,
            exe_dir.as_deref(),
            std::env::var("DATA_DIR").ok().as_deref(),
            std::env::var("DATABASE_URL").ok().as_deref(),
            std::env::var("STATIC_DIR").ok().as_deref(),
        );

        let jwt_secret = Zeroizing::new(std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            panic!(
                "FATAL: JWT_SECRET environment variable is not set. \
                 Generate one with: openssl rand -hex 32"
            )
        }));

        if jwt_secret.len() < 32 {
            panic!(
                "FATAL: JWT_SECRET must be at least 32 bytes (256 bits), got {} bytes.",
                jwt_secret.len()
            );
        }

        let access_token_ttl_secs = std::env::var("ACCESS_TOKEN_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(900);

        let refresh_token_ttl_secs = std::env::var("REFRESH_TOKEN_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(604_800);

        let cookie_secure = std::env::var("COOKIE_SECURE")
            .map(|v| v.to_lowercase() != "false")
            .unwrap_or(true);

        let auto_purge_archived_clients = std::env::var("AUTO_PURGE_ARCHIVED_CLIENTS")
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false);

        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

        let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| format!("http://{bind_addr}"));

        let scoped_token_ttl_secs = std::env::var("SCOPED_TOKEN_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(86_400); // 24 h

        let magic_link_ttl_secs = std::env::var("MAGIC_LINK_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(86_400); // 24 h — long enough for async email delivery

        let smtp_host = std::env::var("SMTP_HOST").ok();
        let smtp_port = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(587u16);
        let smtp_user = std::env::var("SMTP_USER").ok();
        let smtp_password = std::env::var("SMTP_PASSWORD").ok().map(Zeroizing::new);
        let smtp_from = std::env::var("SMTP_FROM").ok();
        let smtp_tls = match std::env::var("SMTP_TLS")
            .unwrap_or_else(|_| "starttls".into())
            .to_lowercase()
            .as_str()
        {
            "starttls" => SmtpTls::Starttls,
            "none" => SmtpTls::None,
            other => panic!(
                "FATAL: unknown SMTP_TLS value '{other}'. \
                 Valid values: starttls (default), none (dev only)."
            ),
        };

        let rate_limit_auth_rps = std::env::var("RATE_LIMIT_AUTH_RPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5u32);
        let rate_limit_auth_burst = std::env::var("RATE_LIMIT_AUTH_BURST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10u32);
        let rate_limit_report_rps = std::env::var("RATE_LIMIT_REPORT_RPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.2f64);
        let rate_limit_report_burst = std::env::var("RATE_LIMIT_REPORT_BURST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3u32);

        let desk_email = std::env::var("DESK_EMAIL").unwrap_or_else(|_| "desk@local".to_string());

        let email_hash_salt = Zeroizing::new(
            std::env::var("EMAIL_HASH_SALT")
                .unwrap_or_else(|_| panic!("FATAL: EMAIL_HASH_SALT not set")),
        );

        // Prevent magic links being generated over plain HTTP in production.
        if cookie_secure && base_url.starts_with("http://") {
            panic!(
                "FATAL: COOKIE_SECURE=true but BASE_URL starts with 'http://'. \
                 Magic links would be delivered over plain HTTP, exposing one-time tokens. \
                 Fix: set BASE_URL=https://your-domain.com  \
                 (or set COOKIE_SECURE=false for local HTTP development only)."
            );
        }

        Ok(Config {
            database: paths.database,
            data_dir: paths.data_dir,
            data_dir_explicit: paths.data_dir_explicit,
            static_dir: paths.static_dir,
            auto_purge_archived_clients,
            jwt_secret,
            access_token_ttl_secs,
            refresh_token_ttl_secs,
            cookie_secure,
            bind_addr,
            base_url,
            scoped_token_ttl_secs,
            magic_link_ttl_secs,
            smtp_host,
            smtp_port,
            smtp_user,
            smtp_password,
            smtp_from,
            smtp_tls,
            rate_limit_auth_rps,
            rate_limit_auth_burst,
            rate_limit_report_rps,
            rate_limit_report_burst,
            desk_email,
            email_hash_salt,
        })
    }

    pub fn encoding_key(&self) -> EncodingKey {
        EncodingKey::from_secret(self.jwt_secret.as_bytes())
    }

    pub fn decoding_key(&self) -> DecodingKey {
        DecodingKey::from_secret(self.jwt_secret.as_bytes())
    }
}
