//! Typed configuration loaded once from environment variables at startup.

use std::env;

pub struct Config {
    /// Address the SMTP server binds to (e.g. `0.0.0.0:25`).
    pub smtp_addr: String,

    /// Address the HTTP API server binds to (e.g. `0.0.0.0:8080`).
    pub http_addr: String,

    /// How often expired mailboxes are purged, in seconds.
    pub cleanup_interval_secs: u64,

    /// MQTT broker URL (e.g. `mqtt://localhost:1883`).
    /// Set to `None` to disable MQTT publishing.
    pub mqtt_broker_url: Option<String>,
}

impl Config {
    /// Build a [`Config`] from the current process environment.
    pub fn from_env() -> Self {
        Self {
            smtp_addr: env::var("SMTP_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:25".to_string()),
            http_addr: env::var("HTTP_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            cleanup_interval_secs: env::var("MAILBOX_CLEANUP_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            mqtt_broker_url: env::var("MQTT_BROKER_URL").ok(),
        }
    }
}
