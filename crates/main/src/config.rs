//! Typed configuration loaded once from environment variables at startup.

use std::env;

pub struct Config {
    /// Address the SMTP server binds to (e.g. `0.0.0.0:25`).
    pub smtp_addr: String,

    /// Address the HTTP API server binds to (e.g. `0.0.0.0:8080`).
    pub http_addr: String,

    /// The mail domain used in SMTP greetings and generated email addresses.
    pub mail_domain: String,

    /// How often expired mailboxes are purged, in seconds.
    pub cleanup_interval_secs: u64,

    /// MQTT broker URL (e.g. `mqtt://localhost:1883`).
    /// Set to `None` to disable MQTT publishing.
    pub mqtt_broker_url: Option<String>,

    /// URL of the ML sidecar (e.g. `http://ml-sidecar:9000`).
    /// Set to `None` to disable ML enrichment.
    pub ml_sidecar_url: Option<String>,

    /// Path to the TLS certificate PEM file (e.g. `/etc/letsencrypt/live/…/fullchain.pem`).
    /// Both cert and key must be set to enable STARTTLS.
    pub smtp_tls_cert: Option<String>,

    /// Path to the TLS private key PEM file (e.g. `/etc/letsencrypt/live/…/privkey.pem`).
    pub smtp_tls_key: Option<String>,

    /// Path to the DKIM private key PEM file (PKCS#8 RSA, set with `dkim_selector` to enable sending).
    pub dkim_private_key_path: Option<String>,

    /// DKIM selector — must match the DNS TXT record `{selector}._domainkey.{domain}`.
    pub dkim_selector: Option<String>,

    /// Maximum total concurrent SMTP connections.
    pub smtp_max_connections: usize,

    /// Maximum concurrent SMTP connections from a single IP address.
    pub smtp_max_connections_per_ip: usize,
}

impl Config {
    /// Build a [`Config`] from the current process environment.
    pub fn from_env() -> Self {
        Self {
            smtp_addr: env::var("SMTP_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:25".to_string()),
            http_addr: env::var("HTTP_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            mail_domain: env::var("MAIL_DOMAIN")
                .unwrap_or_else(|_| "localhost".to_string()),
            cleanup_interval_secs: env::var("MAILBOX_CLEANUP_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            mqtt_broker_url: env::var("MQTT_BROKER_URL").ok(),
            ml_sidecar_url: env::var("ML_SIDECAR_URL").ok(),
            smtp_tls_cert: env::var("SMTP_TLS_CERT").ok(),
            smtp_tls_key: env::var("SMTP_TLS_KEY").ok(),
            dkim_private_key_path: env::var("DKIM_PRIVATE_KEY_PATH").ok(),
            dkim_selector: env::var("DKIM_SELECTOR").ok(),
            smtp_max_connections: env::var("SMTP_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            smtp_max_connections_per_ip: env::var("SMTP_MAX_CONNECTIONS_PER_IP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
        }
    }
}
