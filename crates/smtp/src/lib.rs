//! Phantom Mail — SMTP server crate.

pub(crate) mod connection;
pub(crate) mod parser;
pub(crate) mod rate_limiter;
mod server;
pub(crate) mod store;

pub use server::{load_tls_acceptor, SmtpServer};

