// Database management and interactions

use tokio_postgres::{Client, NoTls, Error};
use tracing::{info, error};
use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use flux_types::{Email, TemporaryMailbox};

#[derive(Clone)]
pub struct Database {
    client: Arc<Client>,
}

impl Database {
    pub async fn new() -> Result<Self, Error> {
        dotenv().ok();
        
        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set in .env file");
            
        info!("Connecting to database...");
        let (client, connection) = tokio_postgres::connect(&database_url, NoTls).await?;
        let client = Arc::new(client);
        
        // Spawn the connection handler
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!("Connection error: {}", e);
            }
        });
        
        // Ensure the database is initialized
        Self::init_database(client.as_ref()).await?;
        
        Ok(Self { client })
    }
    
    async fn init_database(client: &Client) -> Result<(), Error> {
        info!("Initializing database tables...");
        
        // Create tables if they don't exist
        client.batch_execute("
            CREATE TABLE IF NOT EXISTS mailboxes (
                email_address TEXT PRIMARY KEY,
                created_at TIMESTAMPTZ NOT NULL,
                expires_at TIMESTAMPTZ NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS emails (
                id TEXT PRIMARY KEY,
                sender TEXT NOT NULL,
                recipient TEXT NOT NULL,
                subject TEXT NOT NULL,
                body TEXT NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                FOREIGN KEY (recipient) REFERENCES mailboxes(email_address) ON DELETE CASCADE
            );
        ").await?;
        
        info!("Database initialized successfully");
        Ok(())
    }
    
    // Method to create a temporary mailbox
    pub async fn create_mailbox(&self, mailbox: &TemporaryMailbox) -> Result<(), Error> {
        self.client.execute(
            "INSERT INTO mailboxes (email_address, created_at, expires_at) VALUES ($1, $2, $3)",
            &[&mailbox.email_address, &mailbox.created_at, &mailbox.expires_at],
        ).await?;
        
        Ok(())
    }

    pub async fn mailbox_is_active(&self, email_address: &str) -> Result<bool, Error> {
        let row = self.client.query_one(
            "SELECT EXISTS(SELECT 1 FROM mailboxes WHERE email_address = $1 AND expires_at > NOW())",
            &[&email_address],
        ).await?;

        Ok(row.get(0))
    }
    
    // Method to store an email
    pub async fn store_email(&self, email: &Email) -> Result<(), Error> {
        self.client.execute(
            "INSERT INTO emails (id, sender, recipient, subject, body, timestamp) VALUES ($1, $2, $3, $4, $5, $6)",
            &[&email.id, &email.sender, &email.recipient, &email.subject, &email.body, &email.timestamp],
        ).await?;
        
        Ok(())
    }
    
    // Method to get emails for a mailbox
    pub async fn get_emails_for_mailbox(&self, email_address: &str) -> Result<Vec<Email>, Error> {
        let rows = self.client.query(
            "SELECT id, sender, recipient, subject, body, timestamp FROM emails WHERE recipient = $1 ORDER BY timestamp DESC",
            &[&email_address],
        ).await?;
        
        let emails = rows.iter().map(|row| {
            Email {
                id: row.get(0),
                sender: row.get(1),
                recipient: row.get(2),
                subject: row.get(3),
                body: row.get(4),
                timestamp: row.get(5),
            }
        }).collect();
        
        Ok(emails)
    }
    
    // Method to delete expired mailboxes
    pub async fn delete_expired_mailboxes(&self) -> Result<u64, Error> {
        // Ensure we can delete mailboxes even if an older schema uses a restrictive FK.
        self.client.execute(
            "DELETE FROM emails WHERE recipient IN (SELECT email_address FROM mailboxes WHERE expires_at < NOW())",
            &[],
        ).await?;

        let result = self.client.execute(
            "DELETE FROM mailboxes WHERE expires_at < NOW()",
            &[],
        ).await?;

        Ok(result)
    }
}
