// Simple SMTP server implementation for receiving emails

use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use tracing::{info, error, debug};
use flux_database::Database;
use flux_types::Email;
use chrono::Utc;
use std::time::Duration;
use uuid::Uuid;

pub struct SmtpServer {
    db: Arc<Database>,
}

impl SmtpServer {
    pub fn new(db: Database) -> Self {
        Self {
            db: Arc::new(db),
        }
    }
    
    pub async fn run(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;
        info!("SMTP server listening on {}", addr);
        
        loop {
            match listener.accept().await {
                Ok((mut socket, addr)) => {
                    debug!("New SMTP connection from {}", addr);
                    
                    let db = Arc::clone(&self.db);
                    
                    // Spawn a new task for each connection
                    tokio::spawn(async move {
                        // Send welcome message
                        let welcome = "220 Temp Mail SMTP Service ready\r\n";
                        if let Err(e) = socket.write_all(welcome.as_bytes()).await {
                            error!("Failed to send welcome message: {}", e);
                            return;
                        }
                        
                        // Process SMTP commands
                        // This is a simplified SMTP implementation
                        // A full implementation would handle all SMTP commands properly
                        
                        // TODO: Implement full SMTP protocol handling
                        
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
    
    // Parse and store email from SMTP data
    async fn process_email(&self, from: &str, to: &str, data: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Parse email headers and body
        // This is a simplified implementation
        
        let mut subject = "No Subject";
        let mut body = data;
        
        // Extract subject from headers
        if let Some(subject_idx) = data.to_lowercase().find("subject:") {
            if let Some(end_idx) = data[subject_idx..].find("\r\n") {
                subject = &data[subject_idx + 8..subject_idx + end_idx].trim();
            }
        }
        
        // Extract body
        if let Some(body_idx) = data.find("\r\n\r\n") {
            body = &data[body_idx + 4..];
        }
        
        // Create email object
        let email = Email {
            id: Uuid::new_v4().to_string(),
            sender: from.to_string(),
            recipient: to.to_string(),
            subject: subject.to_string(),
            body: body.to_string(),
            timestamp: Utc::now(),
        };
        
        // Store email in database
        self.db.store_email(&email).await?;
        
        Ok(())
    }
}
