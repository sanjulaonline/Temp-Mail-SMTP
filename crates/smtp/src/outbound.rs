//! Outbound SMTP delivery with DKIM signing.
//! Supports both direct MX delivery and relay via an authenticated SMTP server (e.g. AWS SES).

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use chrono::Utc;
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use mail_auth::common::crypto::{RsaKey, Sha256};
use mail_auth::dkim::DkimSigner;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, info};
use uuid::Uuid;

/// Optional SMTP relay configuration (e.g. AWS SES, SendGrid).
/// When set, outbound mail is routed through the relay instead of direct MX delivery.
pub struct SmtpRelay {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

pub struct OutboundMailer {
    mail_domain: String,
    dkim_selector: String,
    dkim_private_key_pem: String,
    relay: Option<SmtpRelay>,
    web_url: String,
}

impl OutboundMailer {
    pub fn new(mail_domain: String, dkim_selector: String, dkim_private_key_pem: String, web_url: String) -> Self {
        Self { mail_domain, dkim_selector, dkim_private_key_pem, relay: None, web_url }
    }

    pub fn with_relay(mut self, relay: SmtpRelay) -> Self {
        self.relay = Some(relay);
        self
    }

    pub async fn send(
        &self,
        from: &str,
        to: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let raw = build_message(from, to, subject, body, &self.mail_domain, &self.web_url);
        let signed = self.dkim_sign(raw.as_bytes())?;

        match &self.relay {
            Some(relay) => {
                info!("Delivering {} → {} via relay {}:{}", from, to, relay.host, relay.port);
                deliver_via_relay(relay, from, to, &signed, &self.mail_domain).await
            }
            None => {
                let to_domain = to.rsplit('@').next().ok_or("invalid recipient address")?;
                let mx_host = resolve_mx(to_domain).await?;
                info!("Delivering {} → {} via MX {}", from, to, mx_host);
                deliver(&mx_host, from, to, &signed, &self.mail_domain).await
            }
        }
    }

    fn dkim_sign(&self, raw: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let pk = RsaKey::<Sha256>::from_pkcs8_pem(&self.dkim_private_key_pem)?;
        let sig = DkimSigner::from_key(pk)
            .domain(&self.mail_domain)
            .selector(&self.dkim_selector)
            .headers(["From", "To", "Subject", "Date", "Message-ID", "MIME-Version"])
            .sign(raw)?;

        let mut out = format!("{sig}\r\n").into_bytes();
        out.extend_from_slice(raw);
        Ok(out)
    }
}

/// Strip CR and LF so user input cannot inject extra headers.
fn sanitize_header(value: &str) -> String {
    value.replace(['\r', '\n'], "")
}

fn build_message(from: &str, to: &str, subject: &str, body: &str, domain: &str, web_url: &str) -> String {
    let from = sanitize_header(from);
    let to = sanitize_header(to);
    let subject = sanitize_header(subject);
    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S +0000");
    let msg_id = Uuid::new_v4();
    format!(
        "From: {from}\r\n\
         To: {to}\r\n\
         Subject: {subject}\r\n\
         Date: {date}\r\n\
         Message-ID: <{msg_id}@{domain}>\r\n\
         MIME-Version: 1.0\r\n\
         Content-Type: text/plain; charset=UTF-8\r\n\
         \r\n\
         {body}\r\n\
         \r\n\
         --\r\n\
         Sent via Phantom Mail · {web_url}\r\n\
         This email was composed and sent by a user of Phantom Mail.\r\n\
         The service operator is not responsible for its content.\r\n\
         To report abuse: sanjula692@gmail.com · Policy: {web_url}/policy\r\n\
         Created by Sanjula · https://www.sanjula.online\r\n"
    )
}

async fn resolve_mx(domain: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
    let lookup = tokio::time::timeout(Duration::from_secs(10), resolver.mx_lookup(domain))
        .await
        .map_err(|_| "MX lookup timed out")??;

    lookup
        .iter()
        .min_by_key(|r| r.preference())
        .map(|r| r.exchange().to_string().trim_end_matches('.').to_string())
        .ok_or_else(|| format!("no MX records for {domain}").into())
}

/// Deliver through an authenticated relay (EHLO → STARTTLS → AUTH LOGIN → DATA).
async fn deliver_via_relay(
    relay: &SmtpRelay,
    from: &str,
    to: &str,
    message: &[u8],
    our_domain: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let stream = tokio::time::timeout(
        Duration::from_secs(30),
        TcpStream::connect(format!("{}:{}", relay.host, relay.port)),
    )
    .await
    .map_err(|_| "connection to relay timed out")??;

    let (r, w) = stream.into_split();
    let mut reader = BufReader::new(r);
    let mut writer = w;

    expect(&mut reader, "220").await?;

    send_cmd(&mut writer, &format!("EHLO {our_domain}")).await?;
    read_multiline(&mut reader).await?;

    // Relay connections require STARTTLS before AUTH
    send_cmd(&mut writer, "STARTTLS").await?;
    expect(&mut reader, "220").await?;

    let tcp = reader
        .into_inner()
        .reunite(writer)
        .map_err(|_| "failed to reunite TCP halves")?;

    let connector = build_tls_connector()?;
    let server_name = rustls::pki_types::ServerName::try_from(relay.host.clone())?;
    let tls = connector.connect(server_name, tcp).await?;
    let (tr, mut tw) = tokio::io::split(tls);
    let mut tr = BufReader::new(tr);

    send_cmd(&mut tw, &format!("EHLO {our_domain}")).await?;
    read_multiline(&mut tr).await?;

    // AUTH LOGIN
    send_cmd(&mut tw, "AUTH LOGIN").await?;
    expect(&mut tr, "334").await?;
    let enc = base64::engine::general_purpose::STANDARD;
    tw.write_all(format!("{}\r\n", enc.encode(&relay.username)).as_bytes()).await?;
    expect(&mut tr, "334").await?;
    tw.write_all(format!("{}\r\n", enc.encode(&relay.password)).as_bytes()).await?;
    expect(&mut tr, "235").await?;

    send_mail(&mut tr, &mut tw, from, to, message).await
}

/// Direct MX delivery (plain TCP port 25, optional STARTTLS).
async fn deliver(
    mx_host: &str,
    from: &str,
    to: &str,
    message: &[u8],
    our_domain: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let stream = tokio::time::timeout(
        Duration::from_secs(30),
        TcpStream::connect(format!("{mx_host}:25")),
    )
    .await
    .map_err(|_| "connection to MX timed out")??;

    let (r, w) = stream.into_split();
    let mut reader = BufReader::new(r);
    let mut writer = w;

    expect(&mut reader, "220").await?;

    send_cmd(&mut writer, &format!("EHLO {our_domain}")).await?;
    let caps = read_multiline(&mut reader).await?;
    let has_starttls = caps.iter().any(|l| l.to_ascii_uppercase().contains("STARTTLS"));

    if has_starttls {
        send_cmd(&mut writer, "STARTTLS").await?;
        expect(&mut reader, "220").await?;

        let tcp = reader
            .into_inner()
            .reunite(writer)
            .map_err(|_| "failed to reunite TCP halves")?;

        let connector = build_tls_connector()?;
        let server_name = rustls::pki_types::ServerName::try_from(mx_host.to_owned())?;
        let tls = connector.connect(server_name, tcp).await?;
        let (tr, mut tw) = tokio::io::split(tls);
        let mut tr = BufReader::new(tr);

        send_cmd(&mut tw, &format!("EHLO {our_domain}")).await?;
        read_multiline(&mut tr).await?;

        send_mail(&mut tr, &mut tw, from, to, message).await
    } else {
        send_mail(&mut reader, &mut writer, from, to, message).await
    }
}

async fn send_mail<R, W>(
    reader: &mut BufReader<R>,
    writer: &mut W,
    from: &str,
    to: &str,
    message: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    send_cmd(writer, &format!("MAIL FROM:<{from}>")).await?;
    expect(reader, "250").await?;

    send_cmd(writer, &format!("RCPT TO:<{to}>")).await?;
    expect(reader, "250").await?;

    send_cmd(writer, "DATA").await?;
    expect(reader, "354").await?;

    dot_stuff(writer, message).await?;

    expect(reader, "250").await?;

    send_cmd(writer, "QUIT").await?;
    Ok(())
}

async fn dot_stuff<W: AsyncWrite + Unpin>(
    writer: &mut W,
    message: &[u8],
) -> Result<(), std::io::Error> {
    for line in message.split(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line == b"." {
            writer.write_all(b"..").await?;
        } else {
            writer.write_all(line).await?;
        }
        writer.write_all(b"\r\n").await?;
    }
    writer.write_all(b".\r\n").await
}

async fn send_cmd<W: AsyncWrite + Unpin>(writer: &mut W, cmd: &str) -> Result<(), std::io::Error> {
    debug!("SMTP >> {}", cmd);
    writer.write_all(format!("{cmd}\r\n").as_bytes()).await
}

async fn expect<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    code: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    debug!("SMTP << {}", line.trim());
    if !line.starts_with(code) {
        return Err(format!("expected {code}, got: {}", line.trim()).into());
    }
    Ok(line)
}

async fn read_multiline<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        debug!("SMTP << {}", line.trim());
        let done = line.len() >= 4 && line.as_bytes().get(3) == Some(&b' ');
        lines.push(line);
        if done {
            break;
        }
    }
    Ok(lines)
}

fn build_tls_connector() -> Result<tokio_rustls::TlsConnector, Box<dyn std::error::Error + Send + Sync>> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}
