//! Axum route handlers for the Phantom Mail HTTP API.

use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;

use phantom_types::{ApiResponse, Email, TemporaryMailbox};

use crate::state::AppState;

#[derive(Deserialize)]
pub(crate) struct SendRequest {
    pub to: String,
    pub subject: String,
    pub body: String,
}

// ── Handler shims ────────────────────────────────────────────────────────────

pub(crate) async fn create_mailbox_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> (StatusCode, Json<ApiResponse<TemporaryMailbox>>) {
    if !state.rate_limiter.check(addr.ip()) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Rate limit exceeded — try again in a minute".to_string()),
            }),
        );
    }
    (StatusCode::OK, Json(create_mailbox(&state).await))
}

pub(crate) async fn get_emails_handler(
    Path(email_address): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<Email>>> {
    Json(get_emails(&state, &email_address).await)
}

pub(crate) async fn send_email_handler(
    Path(email_address): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    // Verify the sender mailbox exists and is active
    match state.db.mailbox_is_active(&email_address).await {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse { success: false, data: None, error: Some("Mailbox not found or expired".into()) }),
            );
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
            );
        }
    }

    let Some(mailer) = &state.mailer else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse { success: false, data: None, error: Some("Outbound mail is not configured".into()) }),
        );
    };

    match mailer.send(&email_address, &req.to, &req.subject, &req.body).await {
        Ok(()) => (
            StatusCode::OK,
            Json(ApiResponse { success: true, data: Some(()), error: None }),
        ),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse { success: false, data: None, error: Some(e.to_string()) }),
        ),
    }
}


// ── Business logic ───────────────────────────────────────────────────────────

/// Allocate a new random temporary mailbox (retries on PK collision).
pub(crate) async fn create_mailbox(state: &AppState) -> ApiResponse<TemporaryMailbox> {
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
                // Retry on primary-key collision; bail out on any other error.
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

/// Return all emails for an active mailbox.
pub(crate) async fn get_emails(
    state: &AppState,
    email_address: &str,
) -> ApiResponse<Vec<Email>> {
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
