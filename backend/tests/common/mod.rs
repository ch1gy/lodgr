use std::sync::Arc;

use backend::{config::Config, crypto::EncryptionKey, db, services::auth::hash_password};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;
use uuid::Uuid;
use zeroize::Zeroizing;

pub async fn setup_test_db() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
                .unwrap()
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, dir)
}

pub fn test_config() -> Config {
    Config {
        database_url: "sqlite::memory:".into(),
        jwt_secret: Zeroizing::new("test_secret_at_least_32_bytes_long_abcdef".into()),
        access_token_ttl_secs: 900,
        refresh_token_ttl_secs: 604_800,
        cookie_secure: false,
        bind_addr: "127.0.0.1:3000".into(),
        base_url: "http://localhost:3000".into(),
        scoped_token_ttl_secs: 86_400,
        magic_link_ttl_secs: 3_600,
        smtp_host: None,
        smtp_port: 587,
        smtp_user: None,
        smtp_password: None,
        smtp_from: None,
        rate_limit_auth_rps: 5,
        rate_limit_auth_burst: 10,
        rate_limit_report_rps: 0.2,
        rate_limit_report_burst: 3,
    }
}

/// Fixed 32-byte key — avoids the 2-4 s Argon2 derivation in every test.
pub fn test_enc_key() -> EncryptionKey {
    Arc::new([42u8; 32])
}

/// Create a client user and return (id, email, raw_password).
pub async fn create_test_client(pool: &SqlitePool) -> (String, String, String) {
    let id = Uuid::new_v4().to_string();
    let email = format!("client-{}@test.example", &id[..8]);
    let password = "TestPassword123!".to_owned();
    let hash = hash_password(&password).unwrap();
    db::users::create(pool, &id, "Test Client", &email, &hash, "client")
        .await
        .unwrap();
    (id, email, password)
}

/// Create a desk user and return (id, email, raw_password).
pub async fn create_test_desk(pool: &SqlitePool) -> (String, String, String) {
    let id = Uuid::new_v4().to_string();
    let email = format!("desk-{}@test.example", &id[..8]);
    let password = "DeskPassword456!".to_owned();
    let hash = hash_password(&password).unwrap();
    db::users::create(pool, &id, "Test Desk", &email, &hash, "desk")
        .await
        .unwrap();
    (id, email, password)
}
