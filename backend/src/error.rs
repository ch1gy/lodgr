use axum::{
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

#[derive(Debug)]
pub enum AppError {
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict(String),
    BadRequest(String),
    UnprocessableEntity(String),
    InvalidTransition {
        from: String,
        to: String,
    },
    /// Account lockout. `retry_after_secs = None` means permanent.
    Locked {
        retry_after_secs: Option<u64>,
    },
    TooManyRequests(String),
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Unauthorized => write!(f, "Unauthorized"),
            AppError::Forbidden => write!(f, "Forbidden"),
            AppError::NotFound => write!(f, "Not found"),
            AppError::Conflict(m) => write!(f, "Conflict: {m}"),
            AppError::BadRequest(m) => write!(f, "Bad request: {m}"),
            AppError::UnprocessableEntity(m) => write!(f, "Unprocessable entity: {m}"),
            AppError::InvalidTransition { from, to } => {
                write!(f, "Invalid transition: '{from}' → '{to}'")
            }
            AppError::Locked {
                retry_after_secs: Some(s),
            } => {
                write!(f, "Account locked, retry after {s}s")
            }
            AppError::Locked {
                retry_after_secs: None,
            } => {
                write!(f, "Account permanently locked")
            }
            AppError::TooManyRequests(m) => write!(f, "Too many requests: {m}"),
            AppError::Internal(m) => write!(f, "Internal error: {m}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Locked is the only variant that needs an extra response header.
        if let AppError::Locked { retry_after_secs } = &self {
            let message = match retry_after_secs {
                Some(secs) => {
                    format!("Account is temporarily locked. Try again in {secs} seconds.")
                }
                None => "Account is permanently locked. Contact support to unlock.".into(),
            };
            let mut resp = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({ "error": message })),
            )
                .into_response();
            if let Some(secs) = retry_after_secs {
                if let Ok(v) = HeaderValue::from_str(&secs.to_string()) {
                    resp.headers_mut()
                        .insert(HeaderName::from_static("retry-after"), v);
                }
            }
            return resp;
        }

        let (status, message) = match &self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".into()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden".into()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found".into()),
            AppError::Conflict(m) => (StatusCode::CONFLICT, m.clone()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::UnprocessableEntity(m) => (StatusCode::UNPROCESSABLE_ENTITY, m.clone()),
            AppError::InvalidTransition { .. } => {
                (StatusCode::BAD_REQUEST, "Invalid status transition".into())
            }
            AppError::TooManyRequests(m) => (StatusCode::TOO_MANY_REQUESTS, m.clone()),
            AppError::Internal(m) => {
                tracing::error!("internal error: {m}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".into(),
                )
            }
            AppError::Locked { .. } => unreachable!(),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
