# Phantom Mail

Disposable email service written in Rust. Accepts real Internet email over SMTP, exposes a REST API for creating temporary mailboxes and reading messages, and enriches each email with ML analysis (OTP extraction, spam scoring, category classification).

**Live domain:** `sanjula.online` — deployed on AWS EC2 via Docker Compose.

---

## Contents

- [Features](#features)
- [Quick Start](#quick-start)
- [HTTP API](#http-api)
- [ML Enrichment](#ml-enrichment)
- [MQTT Events](#mqtt-events)
- [Configuration](#configuration)
- [Architecture](#architecture)
- [Database Schema](#database-schema)
- [Testing](#testing)
- [AWS Deployment](#aws-deployment)
- [Project Structure](#project-structure)

---

## Features

- Temporary email addresses with 24-hour TTL (10-char random local-part)
- Real SMTP email reception (port 25) with STARTTLS/TLS upgrade support
- Per-IP and global connection rate limiting
- REST API to create mailboxes and retrieve emails
- Automatic mailbox and email expiry with cascading cleanup
- ML sidecar: OTP code extraction, spam scoring, email category classification
- Optional real-time MQTT event publishing on email arrival
- Optional Next.js frontend

---

## Quick Start

### Docker (recommended)

```bash
cp .env.example .env          # set MAIL_DOMAIN, DATABASE_URL, etc.
cd docker
docker compose up --build
```

Starts PostgreSQL on `:5432`, Mosquitto on `:1883`/`:9001`, ML sidecar on `:9000`, and Phantom Mail on `:25` (SMTP) + `:8080` (HTTP API).

### Local development

```bash
cp .env.example .env
cargo run -p phantom-main
```

---

## HTTP API

### Create a mailbox

```
POST /mailboxes
```

```bash
curl -X POST http://localhost:8080/mailboxes
```

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

### Fetch emails

```
GET /mailboxes/{email}/emails
```

```bash
curl http://localhost:8080/mailboxes/xkqtfabcde%40sanjula.online/emails
```

```json
{
  "success": true,
  "data": [
    {
      "id":        "550e8400-e29b-41d4-a716-446655440000",
      "sender":    "noreply@github.com",
      "recipient": "xkqtfabcde@sanjula.online",
      "subject":   "Your verification code is 483921",
      "body":      "Use code 483921 to verify your email.",
      "timestamp": "2026-05-13T10:05:00Z",
      "ml_meta": {
        "otp_code":   "483921",
        "spam_score": 0.0,
        "category":   "verification"
      }
    }
  ]
}
```

`ml_meta` is `null` if the ML sidecar is disabled or has not yet processed the email.

### Next.js UI (optional)

```bash
cargo run -p phantom-main   # backend first
cd web-next && npm run dev  # http://localhost:3000
```

Set `BACKEND_HTTP_BASE_URL` in `web-next/.env.local` if the backend runs on a different address.

---

## ML Enrichment

Each received email is sent asynchronously to a Python FastAPI sidecar (port 9000) that runs three analyses:

| Field | What it does |
|-------|-------------|
| `otp_code` | Extracts a one-time password or verification code from the email body using regex patterns (6-digit, 4-digit, alphanumeric codes) |
| `spam_score` | Returns a float `0.0`–`1.0` based on spam phrase matching, excessive punctuation, all-caps subject words, and throwaway sender domains |
| `category` | Classifies into `verification`, `newsletter`, `notification`, `receipt`, or `other` using keyword scoring across subject and body |

The SMTP handler fires the email into an `UnboundedSender<Email>` channel and returns `250 Message accepted` immediately — ML processing never delays the SMTP response. Results are written back to the `ml_meta` column in Postgres.

### Sidecar endpoint

```
POST http://ml-sidecar:9000/analyse
```

```json
// Request
{ "id": "uuid", "sender": "...", "subject": "...", "body": "..." }

// Response
{ "otp_code": "483921", "spam_score": 0.0, "category": "verification" }
```

---

## MQTT Events

When `MQTT_BROKER_URL` is set, Phantom Mail publishes to `phantom/mail/received` on every stored email.

```json
{
  "event":     "email.received",
  "id":        "550e8400-e29b-41d4-a716-446655440000",
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

All values are read from environment variables or a `.env` file.

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | *(required)* | PostgreSQL connection string |
| `SMTP_ADDR` | `0.0.0.0:25` | SMTP listener bind address |
| `HTTP_ADDR` | `0.0.0.0:8080` | HTTP API bind address |
| `MAIL_DOMAIN` | `localhost` | Domain in SMTP greeting, EHLO, and generated addresses |
| `MAILBOX_CLEANUP_INTERVAL_SECS` | `60` | How often expired mailboxes are purged |
| `SMTP_MAX_CONNECTIONS` | `100` | Max total concurrent SMTP connections |
| `SMTP_MAX_CONNECTIONS_PER_IP` | `5` | Max concurrent connections from one IP |
| `SMTP_TLS_CERT` | *(unset)* | Path to TLS cert PEM — enables STARTTLS when set with key |
| `SMTP_TLS_KEY` | *(unset)* | Path to TLS private key PEM |
| `MQTT_BROKER_URL` | *(unset)* | e.g. `mqtt://localhost:1883` — disabled when unset |
| `ML_SIDECAR_URL` | *(unset)* | e.g. `http://localhost:9000` — disabled when unset |
| `WEB_DIR` | `web` | Path to static UI assets |

### Enabling STARTTLS (Let's Encrypt)

```bash
SMTP_TLS_CERT=/etc/letsencrypt/live/sanjula.online/fullchain.pem
SMTP_TLS_KEY=/etc/letsencrypt/live/sanjula.online/privkey.pem
```

Mount the Let's Encrypt directory into the container and set both paths. The server advertises `STARTTLS` in its EHLO response and upgrades the connection transparently on demand.

---

## Architecture

### Crate dependency graph

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

### Runtime diagram

```
Internet senders
     │ TCP :25
     ▼
┌──────────────────────────────────────┐
│  SmtpServer          phantom-smtp    │
│  ├─ RateLimiter  (per-IP + global)   │
│  └─ RFC 5321 state machine           │
│       ├─ mailbox_is_active()  ───────┼──► PostgreSQL
│       ├─ store_received_email() ─────┼──► PostgreSQL
│       ├─ ml_tx.send(email)    ───────┼──► ML channel (async, non-blocking)
│       └─ publish_email_received() ───┼──► MQTT (optional)
└──────────────────────────────────────┘
                                │
                    UnboundedSender<Email>
                                │
                                ▼
                ┌───────────────────────────┐
                │  run_ml_analysis task     │
                │  POST /analyse ───────────┼──► ml-sidecar :9000 (Python)
                │  update_email_ml_meta() ──┼──► PostgreSQL (ml_meta column)
                └───────────────────────────┘

Browser / API clients
     │ TCP :8080
     ▼
┌──────────────────────────────────────┐
│  HttpServer          phantom-http    │
│  POST /mailboxes ────────────────────┼──► PostgreSQL
│  GET  /mailboxes/:email/emails ──────┼──► PostgreSQL
│  GET  /*  (static UI) ───────────────┼──► web/ directory
└──────────────────────────────────────┘

┌──────────────────────────────────────┐
│  Cleanup task  (every N secs)        │
│  delete_expired_mailboxes() ─────────┼──► PostgreSQL
└──────────────────────────────────────┘
```

### SMTP session flow

```
connect
  └─► 220 sanjula.online ESMTP Phantom Mail

EHLO client.example
  └─► 250-sanjula.online
      250-STARTTLS          ← only when TLS is configured
      250-SIZE 10485760
      250 8BITMIME

STARTTLS                    ← optional
  └─► 220 Ready to start TLS
      [TLS handshake — session continues over encrypted stream]

MAIL FROM:<sender@example.com>
  └─► 250 OK

RCPT TO:<abc@sanjula.online>
  └─► mailbox_is_active()?
      ├─ yes → 250 OK
      └─ no  → 550 Mailbox not found or expired

DATA
  └─► 354 End data with <CR><LF>.<CR><LF>
      [message body]
      .
      └─► store_received_email()     → PostgreSQL
          ml_tx.send(email)          → ML sidecar (non-blocking)
          publish_email_received()   → MQTT (if configured)
          → 250 Message accepted

QUIT  →  221 Bye
```

### `phantom-smtp` module structure

```
lib.rs
├── server.rs        — TcpListener loop, rate limiter, TLS acceptor loading
├── connection.rs    — RFC 5321 state machine + STARTTLS upgrade + ML/MQTT dispatch
├── parser.rs        — parse_smtp_path, read_smtp_data (dot-unstuffing), store_received_email
├── rate_limiter.rs  — per-IP and global connection limits (RAII ConnectionPermit)
└── store.rs         — MailStore trait + DynMailStore alias (testability seam)
```

`connection.rs` depends on `DynMailStore` (`Arc<dyn MailStore>`), not `Database` directly. Tests use an in-memory `MockStore` with a real `TcpListener` on a random port — no database or TLS certificates required.

---

## Database Schema

```sql
-- Migration 001
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

-- Migration 002
ALTER TABLE emails ADD COLUMN IF NOT EXISTS ml_meta TEXT;
-- Stored as JSON: { "otp_code": "...", "spam_score": 0.0, "category": "..." }
```

---

## Testing

```bash
cargo test
```

11 tests in `phantom-smtp`, all using `MockStore` + real TCP sockets (no database needed):

| Test | Covers |
|------|--------|
| `greeting_contains_domain` | 220 banner includes configured domain |
| `ehlo_returns_multiline_capabilities` | 250 multi-line response format |
| `ehlo_does_not_advertise_starttls_without_tls` | STARTTLS absent when no cert |
| `helo_response_includes_domain` | HELO fallback response |
| `starttls_returns_502_when_tls_not_configured` | 502 when STARTTLS not available |
| `full_delivery_accepted` | End-to-end EHLO→MAIL→RCPT→DATA→250 |
| `unknown_mailbox_rejected_with_550` | RCPT TO unknown address |
| `data_before_mail_from_returns_503` | Out-of-sequence command rejection |
| `rset_clears_transaction_state` | RSET mid-transaction |
| `parse_smtp_path_handles_brackets_and_params` | Parser edge cases |
| `parse_subject_and_body_extracts_subject` | Subject extraction from headers |

```bash
cd ml-sidecar && pip install -r requirements.txt && pytest test_sidecar.py -v
```

10 Python tests covering OTP extraction patterns, category classification, and spam scoring signals.

---

## AWS Deployment

### DNS and network setup

| Step | Detail |
|------|--------|
| Elastic IP | Assign a static IP — required for stable DNS |
| Security Group | Inbound TCP 25 (SMTP) and 8080 (HTTP) from `0.0.0.0/0` |
| A record | `sanjula.online` → Elastic IP |
| MX record | `sanjula.online` → `sanjula.online.` (priority 10) |
| SPF record | `v=spf1 mx ~all` |
| Reverse DNS | EC2 Console → Elastic IPs → Update reverse DNS → `sanjula.online` |

### Verify after deployment

```bash
# MX lookup
nslookup -type=MX sanjula.online

# Manual SMTP handshake
telnet sanjula.online 25
# Expected: 220 sanjula.online ESMTP Phantom Mail
```

Use [MXToolbox SuperTool](https://mxtoolbox.com/SuperTool.aspx) to validate MX, SPF, and PTR records.

---

## Project Structure

```
phantom-mail/
├── Cargo.toml                    # Workspace
├── .env.example                  # Environment template
│
├── crates/
│   ├── types/                    # Shared domain types (Email, MlMeta, TemporaryMailbox)
│   ├── database/                 # PostgreSQL persistence (tokio-postgres)
│   ├── smtp/                     # SMTP server — RFC 5321 + STARTTLS + rate limiting
│   ├── http/                     # HTTP API (Axum) + static file serving
│   ├── mqtt/                     # MQTT event publisher (rumqttc)
│   └── main/                     # Binary entry point, config, ML analysis task
│
├── ml-sidecar/
│   ├── main.py                   # FastAPI app — POST /analyse, GET /health
│   ├── extractor.py              # OTP regex extraction
│   ├── classifier.py             # Spam scoring + category classification
│   ├── test_sidecar.py           # pytest suite (10 tests)
│   ├── requirements.txt
│   └── Dockerfile
│
├── migrations/
│   ├── 001_init.sql              # Initial schema
│   └── 002_ml_meta.sql           # ml_meta column
│
└── docker/
    ├── Dockerfile                # Multi-stage Rust build
    ├── docker-compose.yml        # App + PostgreSQL + Mosquitto + ML sidecar
    └── mosquitto.conf
```

---

## Requirements

- Rust (latest stable)
- PostgreSQL 14+
- Docker + Docker Compose
- Python 3.12+ (ML sidecar only)
- Mosquitto or any MQTT 3.1.1/5.0 broker (optional)

## Known Gaps

| Gap | Detail |
|-----|--------|
| No SPF/DKIM validation | Inbound mail accepted without sender auth checks — intentional for a disposable inbox receiver |
| Single-node | No horizontal scaling; single `tokio-postgres` connection. Sufficient at current scale |
| Rule-based ML | The sidecar uses keyword/regex heuristics, not a trained model. Accuracy is good for OTP extraction and category classification; spam scoring is approximate |
