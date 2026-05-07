# Phantom Mail

> Temporary Email Service written in Rust. Create disposable email addresses instantly for your temporary needs.

## Features

- Create temporary email addresses
- Receive emails via SMTP
- Access received emails via HTTP API
- Automatic mailbox expiration

## Project Structure

```
flux/
├── crates/
│   ├── database/      # Database interaction layer
│   ├── http/          # HTTP API server
│   ├── smtp/          # SMTP server implementation
│   ├── types/         # Shared data types
│   └── main/          # Application entry point
├── Cargo.toml         # Workspace configuration
└── .env.example       # Example environment configuration
```

## Getting Started

1. Clone the repository
2. Copy `.env.example` to `.env` and update with your configuration
3. Set up PostgreSQL database
4. Build and run the project

```bash
# Build the project
cargo build

# Run the application
cargo run -p flux-main
```

## HTTP API

- `POST /mailboxes` → creates a new temporary mailbox
- `GET /mailboxes/{email_address}/emails` → lists emails for an active mailbox

Examples (defaults to `HTTP_ADDR=0.0.0.0:8080`):

```bash
# Create a mailbox
curl -X POST http://localhost:8080/mailboxes

# Fetch emails (URL-encode the '@' as %40)
curl http://localhost:8080/mailboxes/test%40tempmail.example.com/emails
```

Note: the SMTP server only accepts mail for mailboxes that exist and are not expired.

## Requirements

- Rust (latest stable)
- PostgreSQL
- Docker (optional, for containerized deployment)
