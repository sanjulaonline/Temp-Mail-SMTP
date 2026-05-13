//! HTTP API server — binds the Axum router and exposes a programmatic API.

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use chrono::Duration;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

use phantom_database::Database;
use phantom_types::{ApiResponse, Email, TemporaryMailbox};

use crate::routes::{create_mailbox, create_mailbox_handler, get_emails, get_emails_handler};
use crate::state::AppState;

pub struct HttpServer {
    state: AppState,
}

impl HttpServer {
    pub fn new(db: Database, mail_domain: String) -> Self {
        Self {
            state: AppState {
                db: Arc::new(db),
                mail_domain,
                mailbox_ttl: Duration::hours(24),
            },
        }
    }

    /// Bind to `addr` and serve the HTTP API + static UI.
    pub async fn run(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;
        info!("HTTP server listening on {}", addr);

        let web_dir = std::env::var("WEB_DIR").unwrap_or_else(|_| "web".to_string());

        let app = Router::new()
            .route("/mailboxes", post(create_mailbox_handler))
            .route("/mailboxes/:email_address/emails", get(get_emails_handler))
            .nest_service("/", ServeDir::new(&web_dir)
                .not_found_service(ServeFile::new(format!("{}/index.html", web_dir))))
            .with_state(self.state.clone());

        axum::serve(listener, app).await?;
        Ok(())

    }

    // ── Programmatic API (useful for tests / internal callers) ────────────────

    pub async fn create_temporary_mailbox(
        &self,
    ) -> Result<ApiResponse<TemporaryMailbox>, Box<dyn std::error::Error>> {
        Ok(create_mailbox(&self.state).await)
    }

    pub async fn get_emails(
        &self,
        email_address: &str,
    ) -> Result<ApiResponse<Vec<Email>>, Box<dyn std::error::Error>> {
        Ok(get_emails(&self.state, email_address).await)
    }
}
