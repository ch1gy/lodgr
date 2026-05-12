use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    db,
    error::{AppError, AppResult},
    models::User,
    services::auth::hash_password,
};

pub async fn create_client(
    pool: &SqlitePool,
    name: String,
    email: String,
    password: String,
) -> AppResult<User> {
    // Input length validation
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::BadRequest("name must be 1–100 characters".into()));
    }
    if email.is_empty() || email.len() > 254 {
        return Err(AppError::BadRequest("email must be 1–254 characters".into()));
    }
    if password.len() < 8 {
        return Err(AppError::BadRequest("password must be at least 8 characters".into()));
    }
    if password.len() > 128 {
        return Err(AppError::BadRequest("password must be at most 128 characters".into()));
    }

    let id = Uuid::new_v4().to_string();
    let hash = hash_password(&password)?;

    db::users::create(pool, &id, &name, &email, &hash, "client")
        .await
        .map_err(|e| match e {
            AppError::Internal(ref msg) if msg.contains("UNIQUE") => {
                AppError::Conflict(format!("email '{email}' is already registered"))
            }
            other => other,
        })?;

    db::users::find_by_id(pool, &id)
        .await?
        .ok_or_else(|| AppError::Internal("user vanished after insert".into()))
}

pub async fn delete_client_sessions(pool: &SqlitePool, client_id: &str) -> AppResult<u64> {
    let user = db::users::find_by_id(pool, client_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".to_owned()));
    }

    db::sessions::delete_all_for_user(pool, client_id).await
}
