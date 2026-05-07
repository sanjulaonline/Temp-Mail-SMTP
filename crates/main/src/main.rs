use flux_database::Database;
use flux_http::HttpServer;
use flux_smtp::SmtpServer;
use dotenv::dotenv;
use std::env;
use tracing::{info, error};
use tokio::task;
use tokio::time;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    info!("Starting Temporary Mail Service");
    
    // Load environment variables
    dotenv().ok();
    
    // Initialize database
    let db = match Database::new().await {
        Ok(db) => {
            info!("Database connection established");
            db
        },
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return Err(Box::new(e));
        }
    };
    
    // Start SMTP server
    let smtp_server = SmtpServer::new(db.clone());
    let smtp_addr = env::var("SMTP_ADDR").unwrap_or_else(|_| "0.0.0.0:25".to_string());
    
    let smtp_handle = task::spawn(async move {
        if let Err(e) = smtp_server.run(&smtp_addr).await {
            error!("SMTP server error: {}", e);
        }
    });

    // Periodic cleanup of expired mailboxes
    let cleanup_db = db.clone();
    let cleanup_interval_secs: u64 = env::var("MAILBOX_CLEANUP_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(cleanup_interval_secs));
        loop {
            interval.tick().await;
            match cleanup_db.delete_expired_mailboxes().await {
                Ok(count) if count > 0 => info!("Deleted {} expired mailboxes", count),
                Ok(_) => {}
                Err(e) => error!("Failed to delete expired mailboxes: {}", e),
            }
        }
    });
    
    // Start HTTP server
    let http_server = HttpServer::new(db);
    let http_addr = env::var("HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    
    let http_handle = task::spawn(async move {
        if let Err(e) = http_server.run(&http_addr).await {
            error!("HTTP server error: {}", e);
        }
    });
    
    // Wait for both servers
    let _ = tokio::try_join!(smtp_handle, http_handle);
    
    info!("Shutting down Temporary Mail Service");
    Ok(())
}
