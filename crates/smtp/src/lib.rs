//! Phantom Mail — SMTP server crate.

pub(crate) mod connection;
pub(crate) mod parser;
mod server;
pub(crate) mod store;

pub use server::SmtpServer;

