<div align="center">

# 👻 Phantom Mail

Free, open-source disposable email service built in Rust.

Generate a temporary inbox instantly — no signup, no tracking, no ads.
Send and receive real email with DKIM signing. Auto-expires in 24 hours.

**[phantom-mail.sanjula.online](https://phantom-mail.sanjula.online)** · Built by [Sanjula](https://www.sanjula.online)

</div>

---

- Receive real emails via a full RFC 5321 SMTP server with STARTTLS
- Send from your temp address with DKIM signing
- Mailboxes and all emails auto-deleted after 24 hours
- No account needed — one click, nothing stored about you
- 5 outbound emails per IP + per mailbox per day (VPN-resistant dual limiting)
- Runs on a single AWS t2.micro in ~150 MB RAM

---

## Architecture

### Crate layout

```
phantom-main        binary — wires everything together, starts servers
├── phantom-smtp    SMTP server (RFC 5321 state machine) + MIME parser + outbound mailer
├── phantom-http    Axum HTTP API + rate limiters + CORS + security headers
├── phantom-mqtt    optional MQTT publisher (email arrival events)
├── phantom-database tokio-postgres client, parameterized queries
└── phantom-types   shared structs — Email, TemporaryMailbox, MlMeta
```

### Inbound email flow

```
Internet
  → TCP :25 → SmtpServer
      → connection rate limit (per-IP semaphore, max 5 concurrent / 100 total)
      → spawn tokio task per connection
          → RFC 5321 state machine: EHLO → STARTTLS → MAIL FROM → RCPT TO → DATA
          → 10 MB DATA cap (drops oversized messages)
          → MIME parse (mail-parser): extracts text/plain, decodes quoted-printable
          → store_email() → PostgreSQL
          → ml_tx.send() → ML sidecar task (async, non-blocking)  [optional]
          → MqttPublisher.publish() → MQTT broker                  [optional]
```

### Outbound email flow

```
POST /mailboxes/:address/send
  → IP rate limit   (5 emails / IP / day)
  → mailbox rate limit (5 emails / mailbox / day) ← blocks VPN bypass
  → body size check (50 KB)
  → mailbox_is_active() → PostgreSQL
  → build RFC 2822 message (sanitize headers, add footer)
  → DKIM sign (mail-auth crate, RSA-SHA256)
  → SMTP relay: EHLO → STARTTLS → AUTH LOGIN → DATA  (Resend port 587)
     └─ fallback: direct MX delivery if no relay configured
```

### Optional components

- **ML sidecar** (Python FastAPI) — OTP extraction, spam scoring, email classification. Runs as a separate process; phantom-main sends emails to it over an mpsc channel.
- **MQTT** — publishes an event on every inbound email. Useful for webhooks or real-time notification pipelines.

---

## Quick Start

```bash
git clone https://github.com/sanjulaonline/Temp-Mail-SMTP
cd Temp-Mail-SMTP/docker
cp .env.example .env
docker compose up -d --build
```

SMTP on `:25`, HTTP API on `:8080`.

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | *(required)* | PostgreSQL connection string |
| `MAIL_DOMAIN` | `localhost` | Domain for addresses and SMTP greeting |
| `SMTP_TLS_CERT` / `SMTP_TLS_KEY` | *(unset)* | PEM paths — enables STARTTLS |
| `DKIM_PRIVATE_KEY_PATH` / `DKIM_SELECTOR` | *(unset)* | Enables DKIM signing |
| `SMTP_RELAY_HOST` / `PORT` / `USERNAME` / `PASSWORD` | *(unset)* | SMTP relay (e.g. Resend) |
| `ALLOWED_ORIGIN` | `https://phantom-mail.sanjula.online` | CORS origin |

---

Built by [Sanjula](https://www.sanjula.online) · [sanjula692@gmail.com](mailto:sanjula692@gmail.com) · [Privacy Policy](https://phantom-mail.sanjula.online/policy)
