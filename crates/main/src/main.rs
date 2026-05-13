use dotenv::dotenv;
use tracing::{error, info, warn};

use phantom_database::Database;
use phantom_http::HttpServer;
use phantom_mqtt::MqttPublisher;
use phantom_smtp::SmtpServer;

mod config;
use config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("Starting Phantom Mail Service");

    dotenv().ok();
    let cfg = Config::from_env();

    // ── Database ──────────────────────────────────────────────────────────────
    let db = match Database::new().await {
        Ok(db) => {
            info!("Database connection established");
            db
        }
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return Err(Box::new(e) as Box<dyn std::error::Error>);
        }
    };

    // ── MQTT (optional) ───────────────────────────────────────────────────────
    if let Some(ref broker_url) = cfg.mqtt_broker_url {
        match MqttPublisher::new(broker_url) {
            Ok((_, mut event_loop)) => {
                info!("MQTT publisher connected to {}", broker_url);
                tokio::spawn(async move {
                    loop {
                        if let Err(e) = event_loop.poll().await {
                            warn!("MQTT event-loop error: {}", e);
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        }
                    }
                });
            }
            Err(e) => warn!("MQTT connection failed (continuing without it): {}", e),
        }
    } else {
        info!("MQTT_BROKER_URL not set — MQTT publishing disabled");
    }

    // ── SMTP server ───────────────────────────────────────────────────────────
    let smtp_server = SmtpServer::new(db.clone(), cfg.mail_domain.clone());
    let smtp_addr = cfg.smtp_addr.clone();
    let smtp_handle = tokio::task::spawn(async move {
        if let Err(e) = smtp_server.run(&smtp_addr).await {
            error!("SMTP server error: {}", e);
        }
    });

    // ── Periodic mailbox cleanup ──────────────────────────────────────────────
    let cleanup_db = db.clone();
    let cleanup_secs = cfg.cleanup_interval_secs;
    tokio::task::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(cleanup_secs));
        loop {
            interval.tick().await;
            match cleanup_db.delete_expired_mailboxes().await {
                Ok(n) if n > 0 => info!("Deleted {} expired mailboxes", n),
                Ok(_) => {}
                Err(e) => error!("Failed to delete expired mailboxes: {}", e),
            }
        }
    });

    // ── HTTP API server ───────────────────────────────────────────────────────
    let http_server = HttpServer::new(db, cfg.mail_domain.clone());
    let http_addr = cfg.http_addr.clone();
    let http_handle = tokio::task::spawn(async move {
        if let Err(e) = http_server.run(&http_addr).await {
            error!("HTTP server error: {}", e);
        }
    });

    let _ = tokio::try_join!(smtp_handle, http_handle);

    info!("Shutting down Phantom Mail Service");
    Ok(())
}
