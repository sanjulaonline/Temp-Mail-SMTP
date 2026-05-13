//! Per-connection SMTP session handler.

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error};

use phantom_mqtt::MqttPublisher;
use phantom_types::Email;

use crate::parser::{parse_smtp_path, read_smtp_data, store_received_email};
use crate::store::DynMailStore;

enum SessionOutcome {
    Done,
    UpgradeToTls,
}

/// Entry point for one accepted TCP connection.
/// Performs the STARTTLS upgrade if an acceptor is provided and the client requests it.
pub(crate) async fn handle_smtp_connection(
    store: DynMailStore,
    socket: tokio::net::TcpStream,
    mail_domain: &str,
    tls_acceptor: Option<Arc<tokio_rustls::TlsAcceptor>>,
    publisher: Option<MqttPublisher>,
    ml_tx: Option<UnboundedSender<Email>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let offer_starttls = tls_acceptor.is_some();
    let (read_half, write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);
    let mut writer = write_half;

    match run_smtp_session(
        &store,
        &mut reader,
        &mut writer,
        mail_domain,
        offer_starttls,
        publisher.as_ref(),
        ml_tx.as_ref(),
    )
    .await?
    {
        SessionOutcome::Done => {}
        SessionOutcome::UpgradeToTls => {
            if let Some(acceptor) = tls_acceptor {
                let read_half = reader.into_inner();
                let tcp = read_half
                    .reunite(writer)
                    .map_err(|_| "failed to reunite TCP halves for TLS upgrade")?;

                let tls = acceptor.accept(tcp).await?;
                let (tls_read, tls_write) = tokio::io::split(tls);
                let mut tls_reader = BufReader::new(tls_read);
                let mut tls_writer = tls_write;

                run_smtp_session(
                    &store,
                    &mut tls_reader,
                    &mut tls_writer,
                    mail_domain,
                    false,
                    publisher.as_ref(),
                    ml_tx.as_ref(),
                )
                .await?;
            }
        }
    }

    Ok(())
}

/// RFC 5321 state machine, generic over any async reader/writer pair so it works
/// identically over plain TCP and TLS streams.
///
/// Returns `SessionOutcome::UpgradeToTls` when the client sends STARTTLS and
/// `offer_starttls` is true; the caller performs the handshake and calls this
/// again on the TLS stream.
async fn run_smtp_session<R, W>(
    store: &DynMailStore,
    reader: &mut BufReader<R>,
    writer: &mut W,
    mail_domain: &str,
    offer_starttls: bool,
    publisher: Option<&MqttPublisher>,
    ml_tx: Option<&UnboundedSender<Email>>,
) -> Result<SessionOutcome, Box<dyn std::error::Error + Send + Sync>>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let greeting = format!("{} ESMTP Phantom Mail", mail_domain);
    write_response(writer, 220, &greeting).await?;

    let mut mail_from: Option<String> = None;
    let mut rcpt_to: Vec<String> = Vec::new();

    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).await?;
        if bytes == 0 {
            break;
        }

        let line = line.trim_end_matches(|c| c == '\r' || c == '\n');
        if line.is_empty() {
            continue;
        }

        debug!("SMTP << {}", line);
        let upper = line.to_ascii_uppercase();

        if upper == "QUIT" {
            write_response(writer, 221, "Bye").await?;
            break;
        }

        if upper.starts_with("HELO") {
            let reply = format!("{} OK", mail_domain);
            write_response(writer, 250, &reply).await?;
            continue;
        }

        if upper.starts_with("EHLO") {
            write_ehlo(writer, mail_domain, offer_starttls).await?;
            continue;
        }

        if upper == "STARTTLS" {
            if !offer_starttls {
                write_response(writer, 502, "TLS not available").await?;
                continue;
            }
            write_response(writer, 220, "Ready to start TLS").await?;
            return Ok(SessionOutcome::UpgradeToTls);
        }

        if upper == "NOOP" {
            write_response(writer, 250, "OK").await?;
            continue;
        }

        if upper == "RSET" {
            mail_from = None;
            rcpt_to.clear();
            write_response(writer, 250, "OK").await?;
            continue;
        }

        if upper.starts_with("MAIL FROM:") {
            let raw = line["MAIL FROM:".len()..].trim();
            let addr = parse_smtp_path(raw).unwrap_or_default();
            mail_from = Some(addr);
            rcpt_to.clear();
            write_response(writer, 250, "OK").await?;
            continue;
        }

        if upper.starts_with("RCPT TO:") || upper.starts_with("RCPT TO ") {
            let raw = line["RCPT TO".len()..]
                .trim_start_matches(|c| c == ':' || c == ' ')
                .trim();
            let Some(recipient) = parse_smtp_path(raw) else {
                write_response(writer, 501, "Syntax: RCPT TO:<address>").await?;
                continue;
            };

            match store.mailbox_is_active(&recipient).await {
                Ok(true) => {
                    rcpt_to.push(recipient);
                    write_response(writer, 250, "OK").await?;
                }
                Ok(false) => {
                    write_response(writer, 550, "Mailbox not found or expired").await?;
                }
                Err(e) => {
                    error!("Database error while validating recipient: {}", e);
                    write_response(writer, 451, "Temporary server error").await?;
                }
            }
            continue;
        }

        if upper == "DATA" {
            let Some(from) = mail_from.as_deref() else {
                write_response(writer, 503, "Bad sequence: MAIL FROM required").await?;
                continue;
            };
            if rcpt_to.is_empty() {
                write_response(writer, 503, "Bad sequence: RCPT TO required").await?;
                continue;
            }

            write_response(writer, 354, "End data with <CR><LF>.<CR><LF>").await?;
            let data = read_smtp_data(reader).await?;

            let mut all_ok = true;
            for recipient in &rcpt_to {
                match store_received_email(store.as_ref(), from, recipient, &data).await {
                    Ok(email) => {
                        if let Some(tx) = ml_tx {
                            let _ = tx.send(email.clone());
                        }
                        if let Some(pub_ref) = publisher {
                            pub_ref.publish_email_received(&email).await;
                        }
                    }
                    Err(e) => {
                        error!("Failed to store received email for {}: {}", recipient, e);
                        all_ok = false;
                    }
                }
            }

            mail_from = None;
            rcpt_to.clear();

            if all_ok {
                write_response(writer, 250, "Message accepted").await?;
            } else {
                write_response(writer, 451, "Temporary server error").await?;
            }
            continue;
        }

        write_response(writer, 502, "Command not implemented").await?;
    }

    Ok(SessionOutcome::Done)
}

async fn write_response<W: AsyncWrite + Unpin>(
    writer: &mut W,
    code: u16,
    message: &str,
) -> Result<(), std::io::Error> {
    let response = format!("{} {}\r\n", code, message);
    debug!("SMTP >> {} {}", code, message);
    writer.write_all(response.as_bytes()).await
}

async fn write_ehlo<W: AsyncWrite + Unpin>(
    writer: &mut W,
    domain: &str,
    offer_starttls: bool,
) -> Result<(), std::io::Error> {
    let mut response = format!("250-{}\r\n", domain);
    if offer_starttls {
        response.push_str("250-STARTTLS\r\n");
    }
    response.push_str("250-SIZE 10485760\r\n");
    response.push_str("250 8BITMIME\r\n");
    debug!("SMTP >> 250 EHLO capabilities (starttls={})", offer_starttls);
    writer.write_all(response.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::handle_smtp_connection;
    use crate::store::{DynMailStore, MailStore, StoreError};
    use async_trait::async_trait;
    use phantom_types::Email;
    use std::sync::Arc;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::{TcpListener, TcpStream};

    struct MockStore {
        active: bool,
    }

    #[async_trait]
    impl MailStore for MockStore {
        async fn mailbox_is_active(&self, _addr: &str) -> Result<bool, StoreError> {
            Ok(self.active)
        }
        async fn store_email(&self, _email: &Email) -> Result<(), StoreError> {
            Ok(())
        }
    }

    fn mock(active: bool) -> DynMailStore {
        Arc::new(MockStore { active })
    }

    /// Spin up an in-process SMTP handler and return a connected reader/writer.
    async fn connect(
        store: DynMailStore,
        domain: &'static str,
    ) -> (
        BufReader<tokio::net::tcp::OwnedReadHalf>,
        tokio::net::tcp::OwnedWriteHalf,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            handle_smtp_connection(store, sock, domain, None, None, None)
                .await
                .ok();
        });
        let stream = TcpStream::connect(addr).await.unwrap();
        let (r, w) = stream.into_split();
        (BufReader::new(r), w)
    }

    async fn readline(r: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> String {
        let mut line = String::new();
        r.read_line(&mut line).await.unwrap();
        line
    }

    async fn send(w: &mut tokio::net::tcp::OwnedWriteHalf, cmd: &str) {
        w.write_all(format!("{}\r\n", cmd).as_bytes()).await.unwrap();
    }

    /// Consume the 3-line EHLO response (no TLS configured in tests).
    async fn read_ehlo(r: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) {
        readline(r).await; // 250-domain
        readline(r).await; // 250-SIZE
        readline(r).await; // 250 8BITMIME
    }

    #[tokio::test]
    async fn greeting_contains_domain() {
        let (mut r, mut w) = connect(mock(true), "sanjula.online").await;
        let greeting = readline(&mut r).await;
        assert!(
            greeting.starts_with("220 sanjula.online ESMTP Phantom Mail"),
            "unexpected greeting: {greeting}"
        );
        send(&mut w, "QUIT").await;
    }

    #[tokio::test]
    async fn ehlo_returns_multiline_capabilities() {
        let (mut r, mut w) = connect(mock(true), "sanjula.online").await;
        readline(&mut r).await; // 220

        send(&mut w, "EHLO client.test").await;
        let line1 = readline(&mut r).await;
        let line2 = readline(&mut r).await;
        let line3 = readline(&mut r).await;

        assert!(line1.starts_with("250-sanjula.online"), "line1: {line1}");
        assert!(line2.starts_with("250-SIZE 10485760"), "line2: {line2}");
        assert!(line3.starts_with("250 8BITMIME"), "line3: {line3}");

        send(&mut w, "QUIT").await;
    }

    #[tokio::test]
    async fn helo_response_includes_domain() {
        let (mut r, mut w) = connect(mock(true), "sanjula.online").await;
        readline(&mut r).await; // 220

        send(&mut w, "HELO client.test").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("250 sanjula.online"), "resp: {resp}");

        send(&mut w, "QUIT").await;
    }

    #[tokio::test]
    async fn full_delivery_accepted() {
        let (mut r, mut w) = connect(mock(true), "sanjula.online").await;
        readline(&mut r).await; // 220

        send(&mut w, "EHLO sender.test").await;
        read_ehlo(&mut r).await;

        send(&mut w, "MAIL FROM:<sender@example.com>").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("250"), "MAIL FROM: {resp}");

        send(&mut w, "RCPT TO:<box@sanjula.online>").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("250"), "RCPT TO: {resp}");

        send(&mut w, "DATA").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("354"), "DATA: {resp}");

        w.write_all(b"Subject: Test\r\n\r\nHello world.\r\n.\r\n")
            .await
            .unwrap();
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("250"), "message accepted: {resp}");

        send(&mut w, "QUIT").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("221"), "QUIT: {resp}");
    }

    #[tokio::test]
    async fn unknown_mailbox_rejected_with_550() {
        let (mut r, mut w) = connect(mock(false), "sanjula.online").await;
        readline(&mut r).await; // 220

        send(&mut w, "EHLO sender.test").await;
        read_ehlo(&mut r).await;

        send(&mut w, "MAIL FROM:<sender@example.com>").await;
        readline(&mut r).await; // 250

        send(&mut w, "RCPT TO:<unknown@sanjula.online>").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("550"), "expected 550, got: {resp}");

        send(&mut w, "QUIT").await;
    }

    #[tokio::test]
    async fn data_before_mail_from_returns_503() {
        let (mut r, mut w) = connect(mock(true), "sanjula.online").await;
        readline(&mut r).await; // 220

        send(&mut w, "EHLO sender.test").await;
        read_ehlo(&mut r).await;

        send(&mut w, "DATA").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("503"), "expected 503, got: {resp}");

        send(&mut w, "QUIT").await;
    }

    #[tokio::test]
    async fn rset_clears_transaction_state() {
        let (mut r, mut w) = connect(mock(true), "sanjula.online").await;
        readline(&mut r).await; // 220

        send(&mut w, "EHLO sender.test").await;
        read_ehlo(&mut r).await;

        send(&mut w, "MAIL FROM:<sender@example.com>").await;
        readline(&mut r).await; // 250

        send(&mut w, "RSET").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("250"), "RSET: {resp}");

        send(&mut w, "DATA").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("503"), "expected 503 after RSET, got: {resp}");

        send(&mut w, "QUIT").await;
    }

    #[tokio::test]
    async fn starttls_returns_502_when_tls_not_configured() {
        let (mut r, mut w) = connect(mock(true), "sanjula.online").await;
        readline(&mut r).await; // 220

        send(&mut w, "EHLO sender.test").await;
        read_ehlo(&mut r).await;

        send(&mut w, "STARTTLS").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("502"), "expected 502, got: {resp}");

        send(&mut w, "QUIT").await;
    }

    #[tokio::test]
    async fn ehlo_does_not_advertise_starttls_without_tls() {
        let (mut r, mut w) = connect(mock(true), "sanjula.online").await;
        readline(&mut r).await; // 220

        send(&mut w, "EHLO client.test").await;
        let line1 = readline(&mut r).await;
        let line2 = readline(&mut r).await;
        let line3 = readline(&mut r).await;

        // Must be exactly 3 lines (no STARTTLS line inserted).
        assert!(!line1.contains("STARTTLS"), "unexpected STARTTLS in: {line1}");
        assert!(!line2.contains("STARTTLS"), "unexpected STARTTLS in: {line2}");
        assert!(!line3.contains("STARTTLS"), "unexpected STARTTLS in: {line3}");

        send(&mut w, "QUIT").await;
    }
}
