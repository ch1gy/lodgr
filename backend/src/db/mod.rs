pub mod auth_events;
pub mod desk_profile;
pub mod exports;
pub mod internal_notes;
pub mod invoices;
pub mod jwt_revocations;
pub mod magic_links;
pub mod sessions;
pub mod sub_clients;
pub mod thread;
pub mod tickets;
pub mod users;

use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::error::AppResult;

/// Folds the write-ahead log back into the main database file and truncates
/// it. Until this runs, recent writes live only in `support.db-wal`, so a copy
/// of `support.db` alone would miss them. Harmless to call at any time; if a
/// reader is active SQLite does what it can and returns.
pub async fn checkpoint_wal(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await?;
    Ok(())
}

/// Starts a transaction that takes SQLite's write lock immediately.
///
/// A plain `BEGIN` is deferred: the transaction starts as a reader and only
/// asks for the write lock at its first write. If another connection has
/// written in the meantime, SQLite cannot upgrade it and fails at once with
/// "database is locked", and the busy timeout does not apply. `BEGIN
/// IMMEDIATE` takes the lock up front, so a contending writer waits out the
/// busy timeout instead. Use it for every transaction that writes.
pub async fn begin_write(pool: &SqlitePool) -> AppResult<Transaction<'static, Sqlite>> {
    Ok(pool.begin_with("BEGIN IMMEDIATE").await?)
}
