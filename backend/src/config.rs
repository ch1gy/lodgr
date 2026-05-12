use jsonwebtoken::{DecodingKey, EncodingKey};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub access_token_ttl_secs: i64,
    pub refresh_token_ttl_secs: i64,
    /// Set `; Secure` on the refresh-token cookie. Disable only for plain-HTTP local dev.
    pub cookie_secure: bool,
    /// TCP address the server binds to. Default: 127.0.0.1:3000.
    pub bind_addr: String,
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
                "FATAL: JWT_SECRET must be at least 32 bytes (256 bits), got {} bytes. \
                 Generate a stronger secret with: openssl rand -hex 32",
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

        Ok(Config {
            database_url,
            jwt_secret,
            access_token_ttl_secs,
            refresh_token_ttl_secs,
            cookie_secure,
            bind_addr,
        })
    }

    pub fn encoding_key(&self) -> EncodingKey {
        EncodingKey::from_secret(self.jwt_secret.as_bytes())
    }

    pub fn decoding_key(&self) -> DecodingKey {
        DecodingKey::from_secret(self.jwt_secret.as_bytes())
    }
}
