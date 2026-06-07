use sqlx::SqlitePool;

use crate::{error::AppResult, models::DeskProfile};

pub async fn get(pool: &SqlitePool) -> AppResult<DeskProfile> {
    Ok(sqlx::query_as(
        "SELECT id, name, tagline, email, website, city, phone, vat_number \
         FROM desk_profile WHERE id = 'desk'",
    )
    .fetch_one(pool)
    .await?)
}

pub struct UpdateDeskProfile<'a> {
    pub name: &'a str,
    pub tagline: &'a str,
    pub email: &'a str,
    pub website: &'a str,
    pub city: &'a str,
    pub phone: &'a str,
    pub vat_number: &'a str,
}

pub async fn upsert(pool: &SqlitePool, u: &UpdateDeskProfile<'_>) -> AppResult<DeskProfile> {
    sqlx::query(
        "INSERT INTO desk_profile (id, name, tagline, email, website, city, phone, vat_number) \
         VALUES ('desk', ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET \
           name=excluded.name, tagline=excluded.tagline, email=excluded.email, \
           website=excluded.website, city=excluded.city, phone=excluded.phone, \
           vat_number=excluded.vat_number",
    )
    .bind(u.name)
    .bind(u.tagline)
    .bind(u.email)
    .bind(u.website)
    .bind(u.city)
    .bind(u.phone)
    .bind(u.vat_number)
    .execute(pool)
    .await?;
    get(pool).await
}
