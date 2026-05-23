//! Shared Axum application state for the HTTP crate.

use std::sync::Arc;

use chrono::Duration;

use phantom_database::Database;
use phantom_smtp::OutboundMailer;

use crate::rate_limiter::RateLimiter;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: Arc<Database>,
    pub(crate) mail_domain: String,
    pub(crate) mailbox_ttl: Duration,
    pub(crate) rate_limiter: RateLimiter,
    pub(crate) send_rate_limiter: RateLimiter,
    pub(crate) read_rate_limiter: RateLimiter,
    pub(crate) mailer: Option<Arc<OutboundMailer>>,
}
