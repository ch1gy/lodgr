mod common;

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use backend::{
    config::{resolve_paths, Database},
    db,
    tasks::{clean_old_logs, purge_expired_users},
};

// ── DATA_DIR / STATIC_DIR resolution (O1) ────────────────────────────────────

fn cwd() -> PathBuf {
    std::env::temp_dir().join("lodgr-paths-test-cwd")
}

#[test]
fn unset_data_dir_keeps_the_legacy_layout_but_absolute() {
    let p = resolve_paths(&cwd(), None, None, None, None);

    assert_eq!(p.data_dir, cwd());
    assert!(!p.data_dir_explicit);
    match p.database {
        Database::File(f) => assert_eq!(f, cwd().join("data").join("support.db")),
        Database::Url(u) => panic!("expected a file database, got url {u}"),
    }
}

#[test]
fn data_dir_pins_database_and_working_files() {
    let p = resolve_paths(&cwd(), None, Some("store"), None, None);

    assert_eq!(p.data_dir, cwd().join("store"));
    assert!(p.data_dir_explicit);
    match p.database {
        Database::File(f) => assert_eq!(f, cwd().join("store").join("support.db")),
        Database::Url(u) => panic!("expected a file database, got url {u}"),
    }
}

#[test]
fn absolute_data_dir_is_used_as_given() {
    let abs = std::env::temp_dir().join("lodgr-abs-data");
    let p = resolve_paths(&cwd(), None, Some(abs.to_str().unwrap()), None, None);
    assert_eq!(p.data_dir, abs);
}

#[test]
fn blank_data_dir_counts_as_unset() {
    let p = resolve_paths(&cwd(), None, Some("   "), None, None);
    assert!(!p.data_dir_explicit);
    assert_eq!(p.data_dir, cwd());
}

#[test]
fn explicit_database_url_wins() {
    let p = resolve_paths(
        &cwd(),
        None,
        Some("store"),
        Some("sqlite://elsewhere.db"),
        None,
    );
    match p.database {
        Database::Url(u) => assert_eq!(u, "sqlite://elsewhere.db"),
        Database::File(f) => panic!("expected the explicit url, got file {}", f.display()),
    }
}

#[test]
fn explicit_static_dir_wins_and_is_made_absolute() {
    let p = resolve_paths(&cwd(), None, None, None, Some("site/dist"));
    assert_eq!(p.static_dir, cwd().join("site").join("dist"));
}

#[test]
fn dot_dot_segments_are_resolved_in_configured_paths() {
    let p = resolve_paths(
        &cwd(),
        None,
        Some("../shared/./store"),
        None,
        Some("../site"),
    );

    let parent = cwd().parent().unwrap().to_path_buf();
    assert_eq!(p.data_dir, parent.join("shared").join("store"));
    assert_eq!(p.static_dir, parent.join("site"));
}

#[test]
fn static_dir_is_found_next_to_the_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let dist = dir.path().join("frontend").join("dist");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(dist.join("index.html"), "<html></html>").unwrap();

    let p = resolve_paths(dir.path(), None, None, None, None);

    assert_eq!(p.static_dir, dist);
}

#[test]
fn static_dir_is_found_one_level_up_when_started_from_backend() {
    let dir = tempfile::tempdir().unwrap();
    let backend = dir.path().join("backend");
    std::fs::create_dir_all(&backend).unwrap();
    let dist = dir.path().join("frontend").join("dist");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(dist.join("index.html"), "<html></html>").unwrap();

    let p = resolve_paths(&backend, None, None, None, None);

    assert!(p.static_dir.join("index.html").is_file());
}

#[test]
fn static_dir_is_found_from_the_binary_location() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("somewhere-else");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let debug = dir.path().join("target").join("debug");
    std::fs::create_dir_all(&debug).unwrap();
    let dist = dir.path().join("frontend").join("dist");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(dist.join("index.html"), "<html></html>").unwrap();

    let p = resolve_paths(&elsewhere, Some(&debug), None, None, None);

    assert!(p.static_dir.join("index.html").is_file());
}

#[test]
fn missing_static_dir_falls_back_to_a_path_without_index_html() {
    let dir = tempfile::tempdir().unwrap();
    let p = resolve_paths(dir.path(), None, None, None, None);
    assert!(!p.static_dir.join("index.html").exists());
}

// ── Log retention by age (F30) ───────────────────────────────────────────────

fn touch(path: &Path, age: Duration) {
    let f = std::fs::File::create(path).unwrap();
    f.set_modified(SystemTime::now() - age).unwrap();
}

#[tokio::test]
async fn clean_old_logs_removes_by_age_not_by_count() {
    let dir = tempfile::tempdir().unwrap();
    let day = Duration::from_secs(24 * 3600);

    // 35 old files: a count cap of 30 would have kept most of them.
    for i in 0..35u64 {
        touch(
            &dir.path().join(format!("lodgr.2025-01-{i:02}.log")),
            day * (31 + i as u32),
        );
    }
    touch(&dir.path().join("lodgr.recent.log"), day * 3);
    touch(&dir.path().join("lodgr.today.log"), Duration::ZERO);
    touch(&dir.path().join("notes.txt"), day * 400);

    let removed = clean_old_logs(dir.path(), day * 30).await.unwrap();

    assert_eq!(removed, 35);
    assert!(dir.path().join("lodgr.recent.log").exists());
    assert!(dir.path().join("lodgr.today.log").exists());
    assert!(
        dir.path().join("notes.txt").exists(),
        "files that are not Lodgr logs are never touched"
    );
}

#[tokio::test]
async fn clean_old_logs_ignores_a_missing_directory() {
    let dir = tempfile::tempdir().unwrap();
    let removed = clean_old_logs(&dir.path().join("nope"), Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(removed, 0);
}

// ── WAL checkpoint (O2) ──────────────────────────────────────────────────────

#[tokio::test]
async fn checkpoint_wal_folds_the_log_into_the_database_file() {
    let (pool, dir) = common::setup_test_db().await;
    common::create_test_client(&pool).await;

    let wal = dir.path().join("test.db-wal");
    let before = std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0);
    assert!(
        before > 0,
        "writes should be sitting in the WAL before a checkpoint"
    );

    db::checkpoint_wal(&pool).await.unwrap();

    let after = std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0);
    assert_eq!(after, 0, "TRUNCATE leaves an empty WAL");
}

// ── Write transactions take the lock up front (F15) ──────────────────────────

#[tokio::test]
async fn begin_write_makes_a_second_writer_wait_instead_of_failing() {
    let (pool, _dir) = common::setup_test_db().await;

    let first = db::begin_write(&pool).await.unwrap();

    let second_pool = pool.clone();
    let second = tokio::spawn(async move { db::begin_write(&second_pool).await.map(|_| ()) });

    // While the first writer holds the lock, the second is parked, not failed.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !second.is_finished(),
        "second writer should be waiting for the lock"
    );

    first.commit().await.unwrap();
    second
        .await
        .unwrap()
        .expect("second writer proceeds once the first commits");
}

// ── Archived-client purge is opt-in (F14 follow-up) ──────────────────────────

/// An archived client plus an export record, so a purge does not try to write
/// an export file into the working directory.
async fn archived_client(pool: &sqlx::SqlitePool, archived_days_ago: i64) -> String {
    let (id, _, _) = common::create_test_client(pool).await;
    let archived_at = (chrono::Utc::now() - chrono::Duration::days(archived_days_ago)).to_rfc3339();
    sqlx::query("UPDATE users SET deleted_at = ? WHERE id = ?")
        .bind(archived_at)
        .bind(&id)
        .execute(pool)
        .await
        .unwrap();
    db::exports::create(
        pool,
        &uuid::Uuid::new_v4().to_string(),
        &id,
        "exports/none.json",
    )
    .await
    .unwrap();
    id
}

async fn user_exists(pool: &sqlx::SqlitePool, id: &str) -> bool {
    db::users::find_by_id(pool, id).await.unwrap().is_some()
}

#[tokio::test]
async fn purge_is_off_by_default_and_deletes_nothing() {
    let (pool, _dir) = common::setup_test_db().await;
    let expired = archived_client(&pool, 40).await;

    let purged = purge_expired_users(&pool, &common::test_enc_key(), false).await;

    assert_eq!(purged, 0);
    assert!(
        user_exists(&pool, &expired).await,
        "an archived client past the window must survive while auto-purge is off"
    );
}

#[tokio::test]
async fn purge_when_enabled_deletes_only_clients_past_the_window() {
    let (pool, _dir) = common::setup_test_db().await;
    let expired = archived_client(&pool, 40).await;
    let recent = archived_client(&pool, 5).await;
    let (active, _, _) = common::create_test_client(&pool).await;

    let purged = purge_expired_users(&pool, &common::test_enc_key(), true).await;

    assert_eq!(purged, 1);
    assert!(
        !user_exists(&pool, &expired).await,
        "expired client is purged"
    );
    assert!(
        user_exists(&pool, &recent).await,
        "recently archived client stays"
    );
    assert!(user_exists(&pool, &active).await, "active client stays");
}
