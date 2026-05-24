//! HTTP API server — binds the Axum router and exposes a programmatic API.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use axum::{
    extract::Request,
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use chrono::Duration;
use tokio::net::TcpListener;
use tower_http::{
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
};
use tracing::info;

use phantom_database::Database;
use phantom_smtp::OutboundMailer;
use phantom_types::{ApiResponse, Email, TemporaryMailbox};

use crate::rate_limiter::RateLimiter;
use crate::routes::{create_mailbox, create_mailbox_handler, get_emails, get_emails_handler, send_email_handler};
use crate::state::AppState;

/// 100 KB — large enough for any legitimate API request, stops payload floods.
const MAX_REQUEST_BODY_BYTES: usize = 100 * 1024;

pub struct HttpServer {
    state: AppState,
}

impl HttpServer {
    pub fn new(db: Database, mail_domain: String, mailer: Option<OutboundMailer>) -> Self {
        Self {
            state: AppState {
                db: Arc::new(db),
                mail_domain,
                mailbox_ttl: Duration::hours(24),
                rate_limiter: RateLimiter::new(20, StdDuration::from_secs(60)),
                send_rate_limiter: RateLimiter::new(5, StdDuration::from_secs(60)),
                read_rate_limiter: RateLimiter::new(60, StdDuration::from_secs(60)),
                mailer: mailer.map(Arc::new),
            },
        }
    }

    /// Bind to `addr` and serve the HTTP API + static UI.
    pub async fn run(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;
        info!("HTTP server listening on {}", addr);

        let web_dir = std::env::var("WEB_DIR").unwrap_or_else(|_| "web".to_string());

        let allowed_origin = std::env::var("ALLOWED_ORIGIN")
            .unwrap_or_else(|_| "https://phantom-mail.sanjula.online".to_string());

        let cors = CorsLayer::new()
            .allow_origin(allowed_origin.parse::<HeaderValue>().unwrap_or(HeaderValue::from_static("null")))
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([header::CONTENT_TYPE]);

        let app = Router::new()
            .route("/health", get(|| async { StatusCode::OK }))
            .route("/mailboxes", post(create_mailbox_handler))
            .route("/mailboxes/:email_address/emails", get(get_emails_handler))
            .route("/mailboxes/:email_address/send", post(send_email_handler))
            .nest_service("/", ServeDir::new(&web_dir)
                .not_found_service(ServeFile::new(format!("{}/index.html", web_dir))))
            .with_state(self.state.clone())
            .layer(middleware::from_fn(security_headers))
            .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES))
            .layer(cors);

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
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

async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options",   HeaderValue::from_static("nosniff"));
    h.insert("x-frame-options",          HeaderValue::from_static("DENY"));
    h.insert("referrer-policy",          HeaderValue::from_static("strict-origin-when-cross-origin"));
    h.insert("x-xss-protection",         HeaderValue::from_static("1; mode=block"));
    h.insert("permissions-policy",       HeaderValue::from_static("geolocation=(), microphone=(), camera=()"));
    res
}
