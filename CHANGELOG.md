# Changelog

All notable changes to this project will be documented in this file.

---

## [Unreleased]

### Backend — trivial hardening pass

#### 1. WAL journal mode enabled (`backend/src/main.rs`)
Added `.journal_mode(SqliteJournalMode::Wal)` to `SqliteConnectOptions` at pool
creation. WAL mode allows concurrent reads and a single writer without blocking
reads during writes. It also provides better crash recovery than the default
DELETE journal. One line change; no migration needed.

#### 2. cargo audit — one finding, no action required (`RUSTSEC-2023-0071`)
Ran `cargo audit` against the full dependency tree.

**Finding: Marvin Attack — RSA timing side-channel**
- Crate: `rsa v0.9.10`
- Severity: 5.9 (MEDIUM)
- Advisory: https://rustsec.org/advisories/RUSTSEC-2023-0071
- No fixed version available.

**Risk to this project: effectively zero.**
The `rsa` crate is a transitive dependency via `sqlx-mysql`, which is pulled in
by `sqlx-macros-core` even though this project uses only the `sqlite` feature.
We do not use MySQL, RSA keys, or any code path that would exercise the affected
`rsa` crate. The code is compiled but never invoked.

**Recommendation:** Wait for sqlx to drop `sqlx-mysql` as a mandatory
`macros-core` dependency (being tracked upstream). No action needed today.
If this is a blocker for a security audit, consider pinning `sqlx` to a version
that separates mysql from the macros feature set, once available.

#### 3. COMMON_PASSWORDS → `HashSet` (`backend/src/services/auth.rs`)
Changed the common-password blocklist from a `const &[&str]` slice (O(n) linear
scan) to a `static OnceLock<HashSet<&'static str>>` (O(1) average lookup).
Initialised once on first `validate_password_strength` call. No behaviour change;
purely a performance fix for the hot path of every account-creation request.

#### 4. Connection pool size explicit (`backend/src/main.rs`)
Added `.max_connections(10)` via `SqlitePoolOptions`. Previously the pool used the
sqlx default (which is implementation-defined). Having the limit explicit and
visible in code makes capacity planning easier and prevents unbounded connection
growth if the default ever changes upstream.

#### 5. `ClientResponse` DTO exposes lockout state (`backend/src/dto.rs`)
Added `failed_attempts: i64` and `locked_until: Option<String>` fields to
`ClientResponse`. Previously the admin panel had no way to know if a client was
locked without attempting a login. The frontend admin panel can now:
- Show a 🔒 badge on locked client rows.
- Make the "Unlock" button conditionally prominent.
- No migration needed — columns already exist in the `users` table.

#### 6. Export download-and-delete (`backend/src/routes/admin.rs`)
Export files contain fully decrypted plaintext client data. Previously they
persisted on disk indefinitely after the download. Now, after reading the file
for download, `tokio::fs::remove_file` deletes it immediately. Deletion failures
are logged as errors but do not fail the download response (client already has
the data; preventing a silent loss is not possible at that point). This closes
the HIGH-severity finding from `notes.md`.

---
