# ── Stage 1: build ────────────────────────────────────────────────────────────
FROM rust:1.87-slim AS builder

RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies before copying source
COPY Cargo.toml Cargo.lock ./
COPY crates/types/Cargo.toml      crates/types/Cargo.toml
COPY crates/database/Cargo.toml   crates/database/Cargo.toml
COPY crates/smtp/Cargo.toml       crates/smtp/Cargo.toml
COPY crates/http/Cargo.toml       crates/http/Cargo.toml
COPY crates/mqtt/Cargo.toml       crates/mqtt/Cargo.toml
COPY crates/main/Cargo.toml       crates/main/Cargo.toml

# Create stub lib/main files so Cargo can resolve the dependency graph
RUN mkdir -p crates/types/src      && echo "pub fn _stub() {}" > crates/types/src/lib.rs \
 && mkdir -p crates/database/src   && echo "pub fn _stub() {}" > crates/database/src/lib.rs \
 && mkdir -p crates/smtp/src       && echo "pub fn _stub() {}" > crates/smtp/src/lib.rs \
 && mkdir -p crates/http/src       && echo "pub fn _stub() {}" > crates/http/src/lib.rs \
 && mkdir -p crates/mqtt/src       && echo "pub fn _stub() {}" > crates/mqtt/src/lib.rs \
 && mkdir -p crates/main/src       && echo "fn main() {}" > crates/main/src/main.rs

RUN cargo build --release -p phantom-main 2>/dev/null || true

# Copy real source and build for real
COPY crates/ crates/
RUN touch crates/*/src/*.rs crates/*/src/**/*.rs 2>/dev/null || true
RUN cargo build --release -p phantom-main

# ── Stage 2: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 libcap2-bin curl \
 && rm -rf /var/lib/apt/lists/* \
 && adduser --disabled-password --gecos '' --uid 1001 phantom

WORKDIR /app

COPY --from=builder /app/target/release/phantom-main ./phantom-mail
COPY migrations/ migrations/

# Allow binding to port 25 without root
RUN setcap 'cap_net_bind_service=+ep' ./phantom-mail \
 && chown -R phantom:phantom /app

EXPOSE 25 8080

USER phantom
ENTRYPOINT ["./phantom-mail"]
