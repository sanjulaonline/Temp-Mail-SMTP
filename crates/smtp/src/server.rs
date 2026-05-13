//! SMTP server — accepts connections, enforces rate limits, and dispatches sessions.

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info, warn};

use phantom_database::Database;
use phantom_mqtt::MqttPublisher;
use phantom_types::Email;

use crate::connection::handle_smtp_connection;
use crate::rate_limiter::RateLimiter;
use crate::store::DynMailStore;

pub struct SmtpServer {
    store: DynMailStore,
    mail_domain: Arc<str>,
    tls_acceptor: Option<Arc<tokio_rustls::TlsAcceptor>>,
    rate_limiter: Arc<RateLimiter>,
    publisher: Option<MqttPublisher>,
    ml_tx: Option<UnboundedSender<Email>>,
}

impl SmtpServer {
    pub fn new(
        db: Database,
        mail_domain: String,
        tls_acceptor: Option<Arc<tokio_rustls::TlsAcceptor>>,
        max_connections: usize,
        max_connections_per_ip: usize,
        publisher: Option<MqttPublisher>,
        ml_tx: Option<UnboundedSender<Email>>,
    ) -> Self {
        Self {
            store: Arc::new(db),
            mail_domain: Arc::from(mail_domain.as_str()),
            tls_acceptor,
            rate_limiter: Arc::new(RateLimiter::new(max_connections_per_ip, max_connections)),
            publisher,
            ml_tx,
        }
    }

    /// Bind to `addr` and accept SMTP connections indefinitely.
    pub async fn run(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(addr).await?;
        info!("SMTP server listening on {addr} (tls={})", self.tls_acceptor.is_some());

        loop {
            let (socket, peer_addr) = match listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    continue;
                }
            };

            let peer_ip = peer_addr.ip();
            let permit = match self.rate_limiter.try_acquire(peer_ip) {
                Some(p) => p,
                None => {
                    warn!("Rate limit exceeded for {peer_ip}, dropping connection");
                    // Send 421 before dropping so the sender knows to back off.
                    let _ = socket.writable().await;
                    let _ = socket.try_write(b"421 Too many connections\r\n");
                    continue;
                }
            };

            debug!("New SMTP connection from {peer_addr}");

            let store = self.store.clone();
            let domain = Arc::clone(&self.mail_domain);
            let tls_acceptor = self.tls_acceptor.clone();
            let publisher = self.publisher.clone();
            let ml_tx = self.ml_tx.clone();

            tokio::spawn(async move {
                let _permit = permit;
                if let Err(e) =
                    handle_smtp_connection(store, socket, &domain, tls_acceptor, publisher, ml_tx)
                        .await
                {
                    debug!("SMTP connection {peer_addr} ended with error: {e}");
                }
            });
        }
    }
}

// ── TLS helpers ───────────────────────────────────────────────────────────────

/// Load a `TlsAcceptor` from PEM cert and key files.
/// Accepts Let's Encrypt `fullchain.pem` / `privkey.pem` directly.
pub fn load_tls_acceptor(
    cert_path: &str,
    key_path: &str,
) -> Result<Arc<tokio_rustls::TlsAcceptor>, Box<dyn std::error::Error>> {
    use rustls::ServerConfig;
    use rustls_pemfile::{certs, private_key};
    use std::fs::File;
    use std::io::BufReader;

    let cert_file = File::open(cert_path)
        .map_err(|e| format!("cannot open cert {cert_path}: {e}"))?;
    let key_file = File::open(key_path)
        .map_err(|e| format!("cannot open key {key_path}: {e}"))?;

    let cert_chain: Vec<_> = certs(&mut BufReader::new(cert_file))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("failed to parse cert {cert_path}: {e}"))?;

    let key = private_key(&mut BufReader::new(key_file))
        .map_err(|e| format!("failed to parse key {key_path}: {e}"))?
        .ok_or_else(|| format!("no private key found in {key_path}"))?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .map_err(|e| format!("TLS config error: {e}"))?;

    Ok(Arc::new(tokio_rustls::TlsAcceptor::from(Arc::new(config))))
}
