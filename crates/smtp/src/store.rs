//! Abstraction over the persistence layer used by the SMTP session handler.
//!
//! [`MailStore`] is a thin async trait that the connection handler depends on.
//! The real implementation delegates to [`phantom_database::Database`]; tests
//! supply an in-memory mock without touching PostgreSQL.

use std::sync::Arc;

use async_trait::async_trait;
use phantom_types::Email;

/// Errors returned by [`MailStore`] operations.
pub type StoreError = Box<dyn std::error::Error + Send + Sync>;

/// Minimum database surface needed by the SMTP layer.
#[async_trait]
pub trait MailStore: Send + Sync + 'static {
    /// Return `true` when the mailbox exists **and** has not expired.
    async fn mailbox_is_active(&self, email_address: &str) -> Result<bool, StoreError>;

    /// Persist one inbound e-mail.
    async fn store_email(&self, email: &Email) -> Result<(), StoreError>;
}

// ── Blanket implementation for the real database ──────────────────────────────

#[async_trait]
impl MailStore for phantom_database::Database {
    async fn mailbox_is_active(&self, email_address: &str) -> Result<bool, StoreError> {
        self.mailbox_is_active(email_address)
            .await
            .map_err(|e| Box::new(e) as StoreError)
    }

    async fn store_email(&self, email: &Email) -> Result<(), StoreError> {
        self.store_email(email)
            .await
            .map_err(|e| Box::new(e) as StoreError)
    }
}

/// Convenience alias used throughout the SMTP crate.
pub type DynMailStore = Arc<dyn MailStore>;
