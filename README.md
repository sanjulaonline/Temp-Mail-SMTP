# Phantom Mail

> Temporary Email Service written in Rust. Create disposable email addresses instantly for your temporary needs.

## Features

- Create temporary email addresses
- Receive emails via SMTP
- Access received emails via HTTP API
- Automatic mailbox expiration
- Real-time MQTT event publishing on email arrival

## Project Structure

```
phantom-mail/
├── Cargo.toml                    # Workspace configuration
├── Cargo.lock
├── .env.example                  # Example environment configuration
│
├── crates/
│   ├── types/                    # Shared domain types (Email, TemporaryMailbox, ApiResponse)
│   │   └── src/lib.rs
│   │
│   ├── database/                 # Persistence layer (PostgreSQL via tokio-postgres)
│   │   └── src/
│   │       ├── lib.rs            # Re-exports Database
│   │       └── db.rs             # Database struct and all query methods
│   │
│   ├── smtp/                     # SMTP server (RFC 5321)
│   │   └── src/
│   │       ├── lib.rs            # Re-exports SmtpServer
│   │       ├── server.rs         # SmtpServer — accepts TCP connections
│   │       ├── connection.rs     # Per-connection SMTP state machine
│   │       └── parser.rs         # Path/body parsing, email storage, tests
│   │
│   ├── http/                     # HTTP API server (Axum)
│   │   └── src/
│   │       ├── lib.rs            # Re-exports HttpServer
│   │       ├── server.rs         # HttpServer — binds Axum router
│   │       ├── routes.rs         # Route handlers and business logic
│   │       └── state.rs          # Shared AppState
│   │
│   ├── mqtt/                     # MQTT event publisher (rumqttc)
│   │   └── src/lib.rs            # MqttPublisher — publishes to phantom/mail/received
│   │
│   └── main/                     # Binary entry point
│       └── src/
│           ├── main.rs           # Startup and server wiring
│           └── config.rs         # Typed config from env vars
│
├── migrations/
│   └── 001_init.sql              # Database schema (also applied at startup)
│
└── docker/
    ├── Dockerfile                # Multi-stage Rust build
    ├── docker-compose.yml        # App + PostgreSQL + Mosquitto
    └── mosquitto.conf            # MQTT broker configuration
```

## Getting Started

### Local Development

1. Clone the repository
2. Copy `.env.example` to `.env` and update with your configuration
3. Start PostgreSQL (or use Docker)
4. Optionally start a Mosquitto broker for MQTT events
5. Build and run:

```bash
cargo build
cargo run -p phantom-main
```

### Docker (recommended)

```bash
cd docker
docker compose up --build
```

This starts:
- **PostgreSQL** on port `5432`
- **Mosquitto MQTT broker** on ports `1883` (MQTT) and `9001` (WebSocket)
- **Phantom Mail** on ports `25` (SMTP) and `8080` (HTTP API)

## HTTP API

- `POST /mailboxes` — create a new temporary mailbox
- `GET /mailboxes/{email_address}/emails` — list emails for an active mailbox

```bash
# Create a mailbox
curl -X POST http://localhost:8080/mailboxes

# Fetch emails (URL-encode '@' as %40)
curl http://localhost:8080/mailboxes/test%40tempmail.example.com/emails
```

## MQTT Events

When `MQTT_BROKER_URL` is set, Phantom Mail publishes to **`phantom/mail/received`** on every inbound email:

```json
{
  "event":     "email.received",
  "id":        "uuid-v4",
  "recipient": "user@example.com",
  "sender":    "sender@domain.com",
  "subject":   "Hello",
  "timestamp": "2026-05-08T00:00:00Z"
}
```

Subscribe with any MQTT client, e.g.:

```bash
mosquitto_sub -h localhost -t "phantom/mail/received"
```

## Requirements

- Rust (latest stable)
- PostgreSQL 14+
- Docker + Docker Compose (optional)
- Mosquitto or any MQTT 3.1.1/5.0 broker (optional)
