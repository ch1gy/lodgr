use jsonwebtoken::{DecodingKey, EncodingKey};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
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
    pub smtp_password: Option<String>,
    pub smtp_from: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://data/support.db".to_string());

        let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            panic!(
                "FATAL: JWT_SECRET environment variable is not set. \
                 Generate one with: openssl rand -hex 32"
            )
        });

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

        let bind_addr = std::env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:3000".to_string());

        let base_url = std::env::var("BASE_URL")
            .unwrap_or_else(|_| format!("http://{bind_addr}"));

        let scoped_token_ttl_secs = std::env::var("SCOPED_TOKEN_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(86_400); // 24 h

        let magic_link_ttl_secs = std::env::var("MAGIC_LINK_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3_600); // 1 h

        let smtp_host = std::env::var("SMTP_HOST").ok();
        let smtp_port = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(587u16);
        let smtp_user = std::env::var("SMTP_USER").ok();
        let smtp_password = std::env::var("SMTP_PASSWORD").ok();
        let smtp_from = std::env::var("SMTP_FROM").ok();

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
            database_url,
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
        })
    }

    pub fn encoding_key(&self) -> EncodingKey {
        EncodingKey::from_secret(self.jwt_secret.as_bytes())
    }

    pub fn decoding_key(&self) -> DecodingKey {
        DecodingKey::from_secret(self.jwt_secret.as_bytes())
    }
}
