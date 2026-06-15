use axum::{
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{crypto::EncryptionKey, error::AppResult, middleware::DeskUser, services};

#[derive(Deserialize)]
pub struct RangeQuery {
    pub from: String, // YYYY-MM-DD
    pub to: String,   // YYYY-MM-DD
}

/// GET /reports/range/:client_id?from=YYYY-MM-DD&to=YYYY-MM-DD
pub async fn range(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    _: DeskUser,
    Path(client_id): Path<String>,
    Query(q): Query<RangeQuery>,
) -> AppResult<Response> {
    // Basic validation — dates must look like YYYY-MM-DD
    if q.from.len() != 10 || q.to.len() != 10 || q.from > q.to {
        return Err(crate::error::AppError::BadRequest(
            "from/to must be YYYY-MM-DD and from ≤ to".into(),
        ));
    }

    let report =
        services::reports::range_report(&pool, &enc_key, &client_id, &q.from, &q.to).await?;

    let filename = format!("lodgr-report-{}-{}.pdf", q.from, q.to);
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        report.pdf_bytes,
    )
        .into_response())
}
