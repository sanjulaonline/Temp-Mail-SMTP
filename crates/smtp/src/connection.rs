//! Per-connection SMTP session handler.

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error};

use crate::parser::{parse_smtp_path, read_smtp_data, store_received_email};
use crate::store::DynMailStore;

/// Drive a single accepted TCP connection through the SMTP state machine.
pub(crate) async fn handle_smtp_connection(
    store: DynMailStore,
    socket: tokio::net::TcpStream,
    mail_domain: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (read_half, mut write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);

    let greeting = format!("{} ESMTP Phantom Mail", mail_domain);
    write_response(&mut write_half, 220, &greeting).await?;

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
            write_response(&mut write_half, 221, "Bye").await?;
            break;
        }

        if upper.starts_with("HELO") {
            let reply = format!("{} OK", mail_domain);
            write_response(&mut write_half, 250, &reply).await?;
            continue;
        }

        if upper.starts_with("EHLO") {
            write_ehlo(&mut write_half, mail_domain).await?;
            continue;
        }

        if upper == "NOOP" {
            write_response(&mut write_half, 250, "OK").await?;
            continue;
        }

        if upper == "RSET" {
            mail_from = None;
            rcpt_to.clear();
            write_response(&mut write_half, 250, "OK").await?;
            continue;
        }

        if upper.starts_with("MAIL FROM:") {
            let raw = line["MAIL FROM:".len()..].trim();
            let addr = parse_smtp_path(raw).unwrap_or_default();
            mail_from = Some(addr);
            rcpt_to.clear();
            write_response(&mut write_half, 250, "OK").await?;
            continue;
        }

        if upper.starts_with("RCPT TO:") || upper.starts_with("RCPT TO ") {
            let raw = line["RCPT TO".len()..]
                .trim_start_matches(|c| c == ':' || c == ' ')
                .trim();
            let Some(recipient) = parse_smtp_path(raw) else {
                write_response(&mut write_half, 501, "Syntax: RCPT TO:<address>").await?;
                continue;
            };

            match store.mailbox_is_active(&recipient).await {
                Ok(true) => {
                    rcpt_to.push(recipient);
                    write_response(&mut write_half, 250, "OK").await?;
                }
                Ok(false) => {
                    write_response(&mut write_half, 550, "Mailbox not found or expired").await?;
                }
                Err(e) => {
                    error!("Database error while validating recipient: {}", e);
                    write_response(&mut write_half, 451, "Temporary server error").await?;
                }
            }

            continue;
        }

        if upper == "DATA" {
            let Some(from) = mail_from.as_deref() else {
                write_response(&mut write_half, 503, "Bad sequence: MAIL FROM required").await?;
                continue;
            };

            if rcpt_to.is_empty() {
                write_response(&mut write_half, 503, "Bad sequence: RCPT TO required").await?;
                continue;
            }

            write_response(
                &mut write_half,
                354,
                "End data with <CR><LF>.<CR><LF>",
            )
            .await?;

            let data = read_smtp_data(&mut reader).await?;
            let mut all_ok = true;
            for recipient in &rcpt_to {
                if let Err(e) = store_received_email(store.as_ref(), from, recipient, &data).await {
                    error!("Failed to store received email for {}: {}", recipient, e);
                    all_ok = false;
                }
            }

            mail_from = None;
            rcpt_to.clear();

            if all_ok {
                write_response(&mut write_half, 250, "Message accepted").await?;
            } else {
                write_response(&mut write_half, 451, "Temporary server error").await?;
            }

            continue;
        }

        write_response(&mut write_half, 502, "Command not implemented").await?;
    }

    Ok(())
}

pub(crate) async fn write_response(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    code: u16,
    message: &str,
) -> Result<(), std::io::Error> {
    let response = format!("{} {}\r\n", code, message);
    debug!("SMTP >> {} {}", code, message);
    writer.write_all(response.as_bytes()).await
}

async fn write_ehlo(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    domain: &str,
) -> Result<(), std::io::Error> {
    let response = format!(
        "250-{}\r\n250-SIZE 10485760\r\n250 8BITMIME\r\n",
        domain
    );
    debug!("SMTP >> 250 EHLO capabilities");
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

    /// Spin up an in-process SMTP handler on a random port and return
    /// a line-buffered reader and raw writer connected to it.
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
            handle_smtp_connection(store, sock, domain).await.ok();
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

    // Consume the 3-line EHLO multi-line response.
    async fn read_ehlo(r: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) {
        readline(r).await;
        readline(r).await;
        readline(r).await;
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

        // DATA without MAIL FROM should now fail
        send(&mut w, "DATA").await;
        let resp = readline(&mut r).await;
        assert!(resp.starts_with("503"), "expected 503 after RSET, got: {resp}");

        send(&mut w, "QUIT").await;
    }
}
