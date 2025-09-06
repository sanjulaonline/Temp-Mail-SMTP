# Temporary Mail SMTP Service

A Rust-based temporary email service with SMTP server and HTTP API.

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

## Requirements

- Rust (latest stable)
- PostgreSQL
- Docker (optional, for containerized deployment)
