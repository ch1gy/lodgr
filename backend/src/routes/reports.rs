use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use sqlx::SqlitePool;

use crate::{error::AppResult, middleware::DeskUser, services};

/// GET /reports/monthly/:client_id/:year/:month
pub async fn monthly(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path((client_id, year, month)): Path<(String, i32, u32)>,
) -> AppResult<Response> {
    if month == 0 || month > 12 {
        return Err(crate::error::AppError::BadRequest(
            "month must be 1–12".into(),
        ));
    }

    let report = services::reports::monthly_report(&pool, &client_id, year, month).await?;

    let filename = format!("lodgr-report-{year}-{month:02}.pdf");
    Ok((
        StatusCode::OK,
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
