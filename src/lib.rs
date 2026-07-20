//! # SAPA-AI CRM API
//!
//! A full-featured CRM API built with:
//! - [Axum](https://docs.rs/axum) for HTTP routing and middleware
//! - [Tokio](https://docs.rs/tokio) for the async runtime
//! - [mysql](https://docs.rs/mysql) for MySQL connectivity
//! - [whatsapp-rust](https://github.com/jlucaso1/whatsapp-rust) for in-process WhatsApp Web
//!
//! ## Module map
//! - `config`: reads environment variables and `.env` files.
//! - `database`: MySQL pool initialization, schema, and shared `AppState`.
//! - `error`: a single `AppError` type that every handler can return.
//! - `state`: shared application state injected into requests.
//! - `middleware`: reusable Tower/Axum middleware.
//! - `routes`: route definitions and nesting.
//! - `handlers`: request handlers (thin; delegate to database queries).
//! - `models`: request/response DTOs.
//! - `server`: TCP listener setup and graceful shutdown.
//! - `whatsapp`: in-process WhatsApp Web session registry.
//! - `utils`: helpers and logging macros.

pub mod config;
pub mod database;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod response;
pub mod routes;
pub mod server;
pub mod state;
pub mod utils;
pub mod whatsapp;
