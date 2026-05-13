# Phantom Mail

> Disposable email service written in Rust. Receives real SMTP email from the Internet, exposes a REST API for creating temporary mailboxes and reading their messages.

**Live domain:** `sanjula.online` — deployed on AWS EC2 via Docker Compose.

---

## Features

- Create temporary email addresses (10-char random local-part, 24 h TTL)
- Receive real Internet email via SMTP (port 25) with STARTTLS support
- Per-IP and global SMTP rate limiting
- HTTP API to create mailboxes and fetch emails
- Automatic mailbox expiration + cascading email cleanup
- Optional real-time MQTT event publishing on email arrival

---

## Quick Start

### Docker (recommended)

```bash
cp .env.example .env          # edit MAIL_DOMAIN and DATABASE_URL
cd docker
docker compose up --build
```

Starts PostgreSQL on `:5432`, Mosquitto on `:1883`/`:9001`, and Phantom Mail on `:25` + `:8080`.

### Local development

```bash
cp .env.example .env
cargo run -p phantom-main
```

---

## HTTP API

```bash
# Create a temporary mailbox
curl -X POST http://localhost:8080/mailboxes

# Fetch emails (URL-encode '@' as %40)
curl http://localhost:8080/mailboxes/xkqtfabcde%40sanjula.online/emails
```

**POST /mailboxes**

```json
{
  "success": true,
  "data": {
    "email_address": "xkqtfabcde@sanjula.online",
    "created_at":    "2026-05-13T10:00:00Z",
    "expires_at":    "2026-05-14T10:00:00Z"
  }
}
```

**GET /mailboxes/{email}/emails**

```json
{
  "success": true,
  "data": [
    {
      "id":        "550e8400-...",
      "sender":    "someone@gmail.com",
      "recipient": "xkqtfabcde@sanjula.online",
      "subject":   "Hello",
      "body":      "Hi there.",
      "timestamp": "2026-05-13T10:05:00Z"
    }
  ]
}
```

---

## Next.js UI (optional)

A Next.js port of the UI lives in `web-next/`. It proxies `/mailboxes` requests to the Rust
backend via Next.js rewrites.

```bash
# Start the Rust backend first
cargo run -p phantom-main

# Then start the UI
cd web-next
npm run dev        # http://localhost:3000
```

Set `BACKEND_HTTP_BASE_URL` in `web-next/.env.local` if the backend is not on `http://localhost:8080`.

---

## MQTT Events

When `MQTT_BROKER_URL` is set, Phantom Mail publishes to **`phantom/mail/received`** on every
stored email:

```json
{
  "event":     "email.received",
  "id":        "uuid-v4",
  "recipient": "xkqtfabcde@sanjula.online",
  "sender":    "sender@domain.com",
  "subject":   "Hello",
  "timestamp": "2026-05-13T10:05:00Z"
}
```

```bash
mosquitto_sub -h localhost -t "phantom/mail/received"
```

---

## Configuration

All values are read from environment variables or a `.env` file (`dotenv`).

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | *(required)* | `postgres://user:pass@host:5432/db` |
| `SMTP_ADDR` | `0.0.0.0:25` | Bind address for the SMTP listener |
| `HTTP_ADDR` | `0.0.0.0:8080` | Bind address for the HTTP API |
| `MAIL_DOMAIN` | `localhost` | Domain in SMTP greeting, EHLO, and generated addresses |
| `MAILBOX_CLEANUP_INTERVAL_SECS` | `60` | How often expired mailboxes are purged (seconds) |
| `SMTP_MAX_CONNECTIONS` | `100` | Max total concurrent SMTP connections |
| `SMTP_MAX_CONNECTIONS_PER_IP` | `5` | Max concurrent connections from one IP |
| `SMTP_TLS_CERT` | *(unset)* | Path to TLS cert PEM — enables STARTTLS when set with key |
| `SMTP_TLS_KEY` | *(unset)* | Path to TLS private key PEM |
| `MQTT_BROKER_URL` | *(unset — disabled)* | e.g. `mqtt://localhost:1883` |
| `WEB_DIR` | `web` | Path to static UI assets |

### Enabling STARTTLS (Let's Encrypt)

```bash
# .env or docker-compose.yml
SMTP_TLS_CERT=/etc/letsencrypt/live/sanjula.online/fullchain.pem
SMTP_TLS_KEY=/etc/letsencrypt/live/sanjula.online/privkey.pem
```

Mount the Let's Encrypt directory into the container and set the two paths. The server
advertises `STARTTLS` in its EHLO response and upgrades the connection transparently.

---

## AWS Deployment

### Prerequisites

| Step | What |
|------|------|
| Elastic IP | Assign a static IP — required for stable DNS |
| Security Group | Inbound TCP 25 (SMTP) and 8080 (HTTP) from `0.0.0.0/0` |
| DNS — A record | `sanjula.online` → Elastic IP |
| DNS — MX record | `sanjula.online` → `sanjula.online.` (priority 10) |
| DNS — SPF record | `v=spf1 mx ~all` |
| Reverse DNS (PTR) | EC2 Console → Elastic IPs → Update reverse DNS → `sanjula.online` |

### Verify after deployment

```bash
# MX lookup
nslookup -type=MX sanjula.online

# Manual SMTP handshake
telnet sanjula.online 25
# 220 sanjula.online ESMTP Phantom Mail
```

Use [MXToolbox SuperTool](https://mxtoolbox.com/SuperTool.aspx) to validate MX, SPF, and PTR.

---

## Architecture

### Crate Dependency Graph

```
phantom-main  (binary entry point)
├── phantom-smtp       (SMTP server, port 25)
│   ├── phantom-database
│   ├── phantom-mqtt
│   └── phantom-types
├── phantom-http       (HTTP API + static UI, port 8080)
│   ├── phantom-database
│   └── phantom-types
├── phantom-mqtt       (optional MQTT event publisher)
│   └── phantom-types
└── phantom-database   (PostgreSQL client)
    └── phantom-types

phantom-types  (shared domain structs — no internal deps)
```

### Runtime

```
Internet (MTA senders)
        │  TCP :25
        ▼
┌───────────────────┐
│   SmtpServer      │  phantom-smtp
│                   │  rate limiter (per-IP + global)
│   RFC 5321 FSM    │──► mailbox_is_active()       ──► PostgreSQL
│   per-connection  │──► store_email()              ──► PostgreSQL
│   Tokio task      │──► publish_email_received()   ──► MQTT (optional)
└───────────────────┘

Browser / API client
        │  TCP :8080
        ▼
┌───────────────────┐
│   HttpServer      │  phantom-http  (Axum)
│  POST /mailboxes  │──► create_mailbox()    ──► PostgreSQL
│  GET  /:email/    │──► get_emails()        ──► PostgreSQL
│       emails      │
│  GET  /*          │──► ServeDir  (web/ static files)
└───────────────────┘

┌───────────────────┐
│  Cleanup task     │  tokio::spawn — every MAILBOX_CLEANUP_INTERVAL_SECS
│                   │──► delete_expired_mailboxes() ──► PostgreSQL
└───────────────────┘
```

### SMTP Session Flow

```
connect
  └─► 220 sanjula.online ESMTP Phantom Mail

EHLO client.example
  └─► 250-sanjula.online
      250-STARTTLS          ← only when TLS is configured
      250-SIZE 10485760
      250 8BITMIME

STARTTLS                    ← optional upgrade
  └─► 220 Ready to start TLS
      [TLS handshake — session restarts over encrypted stream]

MAIL FROM:<sender@example.com>  →  250 OK

RCPT TO:<abc@sanjula.online>
  └─► mailbox_is_active()?
      ├─ true  → 250 OK
      └─ false → 550 Mailbox not found or expired

DATA
  └─► 354 End data with <CR><LF>.<CR><LF>
  [message]
  .
  └─► store_received_email() + publish_email_received() → 250 Message accepted

QUIT  →  221 Bye
```

### Database Schema

```sql
CREATE TABLE mailboxes (
    email_address  TEXT        PRIMARY KEY,
    created_at     TIMESTAMPTZ NOT NULL,
    expires_at     TIMESTAMPTZ NOT NULL
);

CREATE TABLE emails (
    id         TEXT        PRIMARY KEY,
    sender     TEXT        NOT NULL,
    recipient  TEXT        NOT NULL REFERENCES mailboxes(email_address) ON DELETE CASCADE,
    subject    TEXT        NOT NULL,
    body       TEXT        NOT NULL,
    timestamp  TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_emails_recipient ON emails (recipient);
CREATE INDEX idx_mailboxes_expires ON mailboxes (expires_at);
```

### `phantom-smtp` Module Structure

```
lib.rs
├── server.rs        — TcpListener loop, rate limiter, TLS acceptor loading
├── connection.rs    — RFC 5321 state machine + STARTTLS upgrade + MQTT publish
├── parser.rs        — parse_smtp_path, read_smtp_data (dot-unstuffing), store_received_email
├── rate_limiter.rs  — per-IP and global connection limits (RAII ConnectionPermit)
└── store.rs         — MailStore trait + DynMailStore alias (testability seam)
```

`connection.rs` depends on `DynMailStore` (`Arc<dyn MailStore>`), not `Database` directly.
Tests use an in-memory `MockStore` and a real `TcpListener` on a random port — no database
or TLS certificates required.

---

## Project Structure

```
phantom-mail/
├── Cargo.toml                    # Workspace configuration
├── .env.example                  # Environment config template
│
├── crates/
│   ├── types/                    # Shared domain types
│   ├── database/                 # PostgreSQL persistence layer
│   ├── smtp/                     # SMTP server (RFC 5321 + STARTTLS)
│   ├── http/                     # HTTP API (Axum)
│   ├── mqtt/                     # MQTT event publisher (rumqttc)
│   └── main/                     # Binary entry point + config
│
├── migrations/
│   └── 001_init.sql              # Schema (also applied at startup)
│
└── docker/
    ├── Dockerfile                # Multi-stage Rust build
    ├── docker-compose.yml        # App + PostgreSQL + Mosquitto
    └── mosquitto.conf            # MQTT broker config
```

---

## Running Tests

```bash
cargo test
```

11 tests covering: SMTP greeting format, EHLO capabilities, HELO, full delivery flow, unknown
mailbox rejection (550), out-of-sequence DATA (503), RSET, STARTTLS unavailable (502), and
EHLO not advertising STARTTLS when TLS is unconfigured.

---

## Requirements

- Rust (latest stable)
- PostgreSQL 14+
- Docker + Docker Compose (optional)
- Mosquitto or any MQTT 3.1.1/5.0 broker (optional)

---

## Known Gaps

| Gap | Detail |
|-----|--------|
| No SPF/DKIM validation | Inbound mail accepted without sender auth checks — intentional for a disposable inbox receiver. |
| Single-node | No horizontal scaling; single `tokio-postgres` client. Sufficient at this scale. |
