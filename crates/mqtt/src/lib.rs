//! Phantom Mail — MQTT event publisher.
//!
//! Publishes a JSON payload to `phantom/mail/received` whenever a new email
//! is stored. The broker URL is read from `MQTT_BROKER_URL` (default:
//! `mqtt://localhost:1883`).

use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::json;
use tracing::{debug, warn};

use phantom_types::Email;

/// Topic used for inbound-email events.
const TOPIC_MAIL_RECEIVED: &str = "phantom/mail/received";

/// A lightweight async MQTT publisher.
///
/// Clone-friendly — the underlying client is already `Arc`-wrapped by rumqttc.
#[derive(Clone)]
pub struct MqttPublisher {
    client: AsyncClient,
}

impl MqttPublisher {
    /// Connect to the MQTT broker and return a publisher + the background event
    /// loop that must be driven to keep the connection alive.
    pub fn new(broker_url: &str) -> Result<(Self, rumqttc::EventLoop), Box<dyn std::error::Error>> {
        let url = broker_url
            .trim_start_matches("mqtt://")
            .trim_start_matches("mqtts://");

        let (host, port) = url.split_once(':').unwrap_or((url, "1883"));
        let port: u16 = port.parse().unwrap_or(1883);

        let mut opts = MqttOptions::new("phantom-mail", host, port);
        opts.set_keep_alive(std::time::Duration::from_secs(30));

        let (client, event_loop) = AsyncClient::new(opts, 64);
        Ok((Self { client }, event_loop))
    }

    /// Publish an `email.received` event for the given email. Failures are
    /// logged as warnings so that the SMTP flow is never blocked by MQTT.
    pub async fn publish_email_received(&self, email: &Email) {
        let payload = json!({
            "event": "email.received",
            "id":        email.id,
            "recipient": email.recipient,
            "sender":    email.sender,
            "subject":   email.subject,
            "timestamp": email.timestamp,
        });

        let payload_bytes = match serde_json::to_vec(&payload) {
            Ok(b) => b,
            Err(e) => {
                warn!("Failed to serialise MQTT payload: {}", e);
                return;
            }
        };

        match self
            .client
            .publish(TOPIC_MAIL_RECEIVED, QoS::AtLeastOnce, false, payload_bytes)
            .await
        {
            Ok(_) => debug!("MQTT published to {}", TOPIC_MAIL_RECEIVED),
            Err(e) => warn!("MQTT publish failed: {}", e),
        }
    }
}
