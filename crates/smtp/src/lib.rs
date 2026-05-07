// Simple SMTP server implementation for receiving emails

use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use tracing::{info, error, debug};
use flux_database::Database;
use flux_types::Email;
use chrono::Utc;
use uuid::Uuid;

pub struct SmtpServer {
    db: Arc<Database>,
}

impl SmtpServer {
    pub fn new(db: Database) -> Self {
        Self {
            db: Arc::new(db),
        }
    }
    
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
            tokio::spawn(async move {
                if let Err(e) = handle_smtp_connection(db, socket).await {
                    debug!("SMTP connection {} ended with error: {}", peer_addr, e);
                }
            });
        }
    }
}

async fn handle_smtp_connection(
    db: Arc<Database>,
    socket: tokio::net::TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (read_half, mut write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);

    write_response(&mut write_half, 220, "Temp Mail SMTP Service ready").await?;

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

async fn write_response(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    code: u16,
    message: &str,
) -> Result<(), std::io::Error> {
    let response = format!("{} {}\r\n", code, message);
    debug!("SMTP >> {} {}", code, message);
    writer.write_all(response.as_bytes()).await
}

async fn read_smtp_data(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut data = String::new();

    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).await?;
        if bytes == 0 {
            return Err("Unexpected EOF while reading DATA".into());
        }

        let stripped = line.trim_end_matches(|c| c == '\r' || c == '\n');
        if stripped == "." {
            break;
        }

        // Dot-stuffing: RFC 5321 section 4.5.2
        let unstuffed = stripped.strip_prefix("..").map(|s| format!(".{}", s)).unwrap_or_else(|| stripped.to_string());
        data.push_str(&unstuffed);
        data.push_str("\r\n");
    }

    Ok(data)
}

fn parse_smtp_path(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    if let Some(start) = raw.find('<') {
        if let Some(end_rel) = raw[start + 1..].find('>') {
            let end = start + 1 + end_rel;
            return Some(raw[start + 1..end].trim().to_string());
        }
    }

    let token = raw.split_whitespace().next().unwrap_or("");
    let token = token.trim_matches(&['<', '>'][..]);
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

async fn store_received_email(
    db: &Database,
    from: &str,
    to: &str,
    data: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (subject, body) = parse_subject_and_body(data);

    let email = Email {
        id: Uuid::new_v4().to_string(),
        sender: from.to_string(),
        recipient: to.to_string(),
        subject,
        body,
        timestamp: Utc::now(),
    };

    db.store_email(&email).await?;
    Ok(())
}

fn parse_subject_and_body(data: &str) -> (String, String) {
    let (headers, body) = data.split_once("\r\n\r\n").unwrap_or((data, ""));

    let mut subject: Option<String> = None;
    for line in headers.split("\r\n") {
        if let Some(prefix) = line.get(..8) {
            if prefix.eq_ignore_ascii_case("subject:") {
                subject = Some(line.get(8..).unwrap_or("").trim().to_string());
                break;
            }
        }
    }

    (
        subject.unwrap_or_else(|| "No Subject".to_string()),
        body.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_smtp_path, parse_subject_and_body};

    #[test]
    fn parse_smtp_path_handles_brackets_and_params() {
        assert_eq!(
            parse_smtp_path("<user@example.com>").as_deref(),
            Some("user@example.com")
        );
        assert_eq!(
            parse_smtp_path("<user@example.com> SIZE=123").as_deref(),
            Some("user@example.com")
        );
        assert_eq!(
            parse_smtp_path("user@example.com").as_deref(),
            Some("user@example.com")
        );
        assert_eq!(parse_smtp_path(""), None);
    }

    #[test]
    fn parse_subject_and_body_extracts_subject() {
        let data = "From: a@example.com\r\nSubject: Hello\r\n\r\nBody line\r\n";
        let (subject, body) = parse_subject_and_body(data);
        assert_eq!(subject, "Hello");
        assert_eq!(body, "Body line\r\n");
    }
}
