//! SMTP path and body parsing helpers, plus email storage.

use chrono::Utc;
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;

use phantom_types::Email;

use crate::store::MailStore;

/// Read the DATA payload until the RFC 5321 end-of-data marker (`\r\n.\r\n`).
pub(crate) async fn read_smtp_data(
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
        let unstuffed = stripped
            .strip_prefix("..")
            .map(|s| format!(".{}", s))
            .unwrap_or_else(|| stripped.to_string());
        data.push_str(&unstuffed);
        data.push_str("\r\n");
    }

    Ok(data)
}

/// Parse an SMTP path like `<user@example.com>` or a bare address.
pub(crate) fn parse_smtp_path(raw: &str) -> Option<String> {
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

/// Extract `Subject` header and body from a raw DATA payload.
pub(crate) fn parse_subject_and_body(data: &str) -> (String, String) {
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

/// Parse and store one inbound email in the database.
pub(crate) async fn store_received_email(
    store: &dyn MailStore,
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

    store.store_email(&email).await?;
    Ok(())
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
