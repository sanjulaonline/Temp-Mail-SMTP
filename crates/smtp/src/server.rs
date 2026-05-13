//! SMTP server — accepts connections and dispatches them to the session handler.

use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::{debug, error, info};

use phantom_database::Database;

use crate::connection::handle_smtp_connection;

pub struct SmtpServer {
    db: Arc<Database>,
    mail_domain: Arc<str>,
}

impl SmtpServer {
    pub fn new(db: Database, mail_domain: String) -> Self {
        Self {
            db: Arc::new(db),
            mail_domain: Arc::from(mail_domain.as_str()),
        }
    }

    /// Bind to `addr` and accept SMTP connections indefinitely.
    pub async fn run(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;
        info!("SMTP server listening on {}", addr);

        loop {
            let (socket, peer_addr) = match listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    continue;
                }
            };

            debug!("New SMTP connection from {}", peer_addr);

            let db = Arc::clone(&self.db);
            let domain = Arc::clone(&self.mail_domain);
            tokio::spawn(async move {
                if let Err(e) = handle_smtp_connection(db, socket, &domain).await {
                    debug!("SMTP connection {} ended with error: {}", peer_addr, e);
                }
            });
        }
    }
}
