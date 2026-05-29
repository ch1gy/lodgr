// Library root — exposes all backend modules so integration tests in tests/
// can import them via `use backend::...`.
pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod dto;
pub mod email;
pub mod error;
pub mod middleware;
pub mod models;
pub mod notify;
pub mod rate_limit;
pub mod routes;
pub mod services;
pub mod tasks;
pub mod ticket_status;
