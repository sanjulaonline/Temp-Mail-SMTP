//! Phantom Mail — SMTP server crate.

/// Greeting sent to clients on connect (RFC 5321 §4.2).
pub(crate) const INITIAL_GREETING: &str = "Phantom Mail Service Ready";

pub(crate) mod connection;
pub(crate) mod parser;
mod server;

pub use server::SmtpServer;

