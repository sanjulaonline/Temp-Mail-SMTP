// HTTP server and API endpoints

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use tokio::net::TcpListener;
use tracing::info;
use std::sync::Arc;
use flux_database::Database;
use flux_types::{TemporaryMailbox, Email, ApiResponse};
use chrono::Utc;
use chrono::Duration;
use std::env;

// HTTP server implementation
pub struct HttpServer {
    state: AppState,
}

#[derive(Clone)]
struct AppState {
    db: Arc<Database>,
    mail_domain: String,
    mailbox_ttl: Duration,
}

impl HttpServer {
    pub fn new(db: Database) -> Self {
        let mail_domain = env::var("MAIL_DOMAIN").unwrap_or_else(|_| "example.com".to_string());

        Self {
            state: AppState {
                db: Arc::new(db),
                mail_domain,
                mailbox_ttl: Duration::hours(24),
            },
        }
    }
    
    pub async fn run(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;
        info!("HTTP server listening on {}", addr);

        let app = Router::new()
            .route("/mailboxes", post(create_mailbox_handler))
            .route("/mailboxes/:email_address/emails", get(get_emails_handler))
            .with_state(self.state.clone());

        axum::serve(listener, app).await?;
        Ok(())
    }
    
    // API method to create a temporary mailbox
    pub async fn create_temporary_mailbox(&self) -> Result<ApiResponse<TemporaryMailbox>, Box<dyn std::error::Error>> {
        Ok(create_mailbox(&self.state).await)
    }
    
    // API method to get emails for a mailbox
    pub async fn get_emails(&self, email_address: &str) -> Result<ApiResponse<Vec<Email>>, Box<dyn std::error::Error>> {
        Ok(get_emails(&self.state, email_address).await)
    }
}

async fn create_mailbox_handler(
    State(state): State<AppState>,
) -> Json<ApiResponse<TemporaryMailbox>> {
    Json(create_mailbox(&state).await)
}

async fn get_emails_handler(
    Path(email_address): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<Email>>> {
    Json(get_emails(&state, &email_address).await)
}

async fn create_mailbox(state: &AppState) -> ApiResponse<TemporaryMailbox> {
    // Retry a few times in the extremely unlikely event of a random collision.
    for _ in 0..5 {
        let local_part: String = (0..10)
            .map(|_| (b'a' + (rand::random::<u8>() % 26)) as char)
            .collect();

        let email_address = format!("{}@{}", local_part, state.mail_domain);
        let now = Utc::now();
        let expires_at = now + state.mailbox_ttl;

        let mailbox = TemporaryMailbox {
            email_address,
            created_at: now,
            expires_at,
        };

        match state.db.create_mailbox(&mailbox).await {
            Ok(_) => {
                return ApiResponse {
                    success: true,
                    data: Some(mailbox),
                    error: None,
                };
            }
            Err(e) => {
                // If we hit a PK collision, retry; otherwise return the error.
                if e.code().map(|c| c.code() == "23505").unwrap_or(false) {
                    continue;
                }

                return ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
            }
        }
    }

    ApiResponse {
        success: false,
        data: None,
        error: Some("Failed to allocate a unique mailbox".to_string()),
    }
}

async fn get_emails(state: &AppState, email_address: &str) -> ApiResponse<Vec<Email>> {
    match state.db.mailbox_is_active(email_address).await {
        Ok(true) => {}
        Ok(false) => {
            return ApiResponse {
                success: false,
                data: None,
                error: Some("Mailbox not found or expired".to_string()),
            };
        }
        Err(e) => {
            return ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
        }
    }

    match state.db.get_emails_for_mailbox(email_address).await {
        Ok(emails) => ApiResponse {
            success: true,
            data: Some(emails),
            error: None,
        },
        Err(e) => ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        },
    }
}
