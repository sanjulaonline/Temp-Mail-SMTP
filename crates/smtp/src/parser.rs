//! SMTP path and body parsing helpers, plus email storage.

use chrono::Utc;
use mail_parser::MessageParser;
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;

use phantom_types::Email;

use crate::store::MailStore;

/// Read the DATA payload until the RFC 5321 end-of-data marker (`\r\n.\r\n`).
pub(crate) async fn read_smtp_data<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
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

/// Extract subject and plain-text body from a raw RFC 5322 / MIME DATA payload.
/// Handles multipart messages, quoted-printable and base64 decoding,
/// and encoded subject headers (=?UTF-8?Q?...?=).
pub(crate) fn parse_subject_and_body(data: &str) -> (String, String) {
    let msg = MessageParser::default().parse(data.as_bytes());

    let subject = msg
        .as_ref()
        .and_then(|m| m.subject())
        .unwrap_or("No Subject")
        .to_string();

    // Prefer text/plain; fall back to stripping tags from text/html.
    let body = msg
        .as_ref()
        .and_then(|m| m.body_text(0))
        .map(|s| s.into_owned())
        .unwrap_or_default();

    (subject, body)
}

/// Parse and store one inbound email. Returns the stored [`Email`] so callers
/// can forward it to event publishers (e.g. MQTT) without re-building the struct.
pub(crate) async fn store_received_email(
    store: &dyn MailStore,
    from: &str,
    to: &str,
    data: &str,
) -> Result<Email, Box<dyn std::error::Error + Send + Sync>> {
    let (subject, body) = parse_subject_and_body(data);

    let email = Email {
        id: Uuid::new_v4().to_string(),
        sender: from.to_string(),
        recipient: to.to_string(),
        subject,
        body,
        timestamp: Utc::now(),
        ml_meta: None,
    };

    store.store_email(&email).await?;
    Ok(email)
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
        assert!(body.contains("Body line"), "body: {body:?}");
    }

    #[test]
    fn parse_subject_and_body_handles_multipart() {
        let data = "From: a@example.com\r\n\
            Subject: Test\r\n\
            MIME-Version: 1.0\r\n\
            Content-Type: multipart/alternative; boundary=\"bound\"\r\n\
            \r\n\
            --bound\r\n\
            Content-Type: text/plain; charset=\"UTF-8\"\r\n\
            \r\n\
            Plain text here\r\n\
            --bound\r\n\
            Content-Type: text/html; charset=\"UTF-8\"\r\n\
            \r\n\
            <p>HTML here</p>\r\n\
            --bound--\r\n";
        let (subject, body) = parse_subject_and_body(data);
        assert_eq!(subject, "Test");
        assert!(body.contains("Plain text here"), "body: {body:?}");
        assert!(!body.contains("<p>"), "should not contain HTML tags: {body:?}");
    }
}
