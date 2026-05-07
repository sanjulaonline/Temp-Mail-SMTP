//! Shared Axum application state for the HTTP crate.

use std::sync::Arc;

use chrono::Duration;

use phantom_database::Database;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: Arc<Database>,
    pub(crate) mail_domain: String,
    pub(crate) mailbox_ttl: Duration,
}
