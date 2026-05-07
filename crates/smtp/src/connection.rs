//! Per-connection SMTP session handler.

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error};

use phantom_database::Database;

use crate::parser::{parse_smtp_path, read_smtp_data, store_received_email};
use crate::INITIAL_GREETING;

/// Drive a single accepted TCP connection through the SMTP state machine.
pub(crate) async fn handle_smtp_connection(
    db: Arc<Database>,
    socket: tokio::net::TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (read_half, mut write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);

    write_response(&mut write_half, 220, INITIAL_GREETING).await?;

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

        if upper.starts_with("HELO") || upper.starts_with("EHLO") {
            write_response(&mut write_half, 250, "OK").await?;
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

            match db.mailbox_is_active(&recipient).await {
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
                if let Err(e) = store_received_email(&db, from, recipient, &data).await {
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
