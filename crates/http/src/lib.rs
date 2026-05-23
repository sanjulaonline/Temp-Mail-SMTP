//! Phantom Mail — HTTP API server crate.

pub(crate) mod rate_limiter;
pub(crate) mod routes;
pub(crate) mod state;
mod server;

pub use server::HttpServer;
