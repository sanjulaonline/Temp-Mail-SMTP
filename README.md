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

```
phantom-main        binary — config, startup
├── phantom-smtp    SMTP server + outbound mailer + MIME parser
├── phantom-http    HTTP API (Axum) + rate limiting + security headers
├── phantom-database PostgreSQL client
└── phantom-types   shared domain structs
```

**Inbound**
```
Internet → TCP :25 → SMTP state machine (EHLO/STARTTLS/MAIL/RCPT/DATA)
  → MIME parse → store in PostgreSQL
```

**Outbound**
```
POST /mailboxes/:addr/send
  → IP rate limit + mailbox rate limit
  → build RFC 2822 → DKIM sign (RSA-SHA256)
  → Resend SMTP relay (port 587, STARTTLS + AUTH LOGIN)
```

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
