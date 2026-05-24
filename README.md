<div align="center">

# 👻 Phantom Mail

**Free, open-source disposable email service built in Rust.**

Generate a temporary inbox instantly — no signup, no tracking, no ads.
Send and receive real email with DKIM signing. Auto-expires in 24 hours.

**[phantom-mail.sanjula.online](https://phantom-mail.sanjula.online)** · Built by [Sanjula](https://www.sanjula.online)

</div>

---

## Features

- **Receive** real emails via a full RFC 5321 SMTP server with STARTTLS
- **Send** from your temp address with DKIM signing
- **Auto-expire** — mailboxes and all emails deleted after 24 hours
- **No account** — one click, nothing stored about you
- **Rate limited** — 5 outbound emails per IP + per mailbox per day (VPN-resistant)
- **Self-hostable** — runs on a single AWS t2.micro in ~150 MB RAM

---

## Stack

| Layer | Technology |
|---|---|
| SMTP server | Rust + Tokio (RFC 5321 from scratch) |
| HTTP API | Rust + Axum |
| Database | PostgreSQL |
| Frontend | Next.js 15 (Vercel) |
| Outbound relay | Resend SMTP |
| Infrastructure | Docker Compose on AWS EC2 free tier |

---

## Quick Start

```bash
git clone https://github.com/sanjulaonline/Temp-Mail-SMTP
cd Temp-Mail-SMTP/docker
cp .env.example .env   # set POSTGRES_PASSWORD and SMTP_RELAY_PASSWORD
docker compose up -d --build
```

SMTP on `:25`, HTTP API on `:8080`.

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | *(required)* | PostgreSQL connection string |
| `MAIL_DOMAIN` | `localhost` | Domain for addresses and SMTP greeting |
| `PHANTOM_WEB_URL` | `https://phantom-mail.sanjula.online` | URL shown in email footers |
| `SMTP_TLS_CERT` / `SMTP_TLS_KEY` | *(unset)* | PEM paths — enables STARTTLS |
| `DKIM_PRIVATE_KEY_PATH` / `DKIM_SELECTOR` | *(unset)* | Enables DKIM signing |
| `SMTP_RELAY_HOST` / `PORT` / `USERNAME` / `PASSWORD` | *(unset)* | SMTP relay (e.g. Resend) |
| `ALLOWED_ORIGIN` | `https://phantom-mail.sanjula.online` | CORS origin |

---

## Architecture

```
phantom-main        binary — config, startup
├── phantom-smtp    SMTP server + outbound mailer + MIME parser
├── phantom-http    HTTP API (Axum) + rate limiting + security headers
├── phantom-database PostgreSQL client
└── phantom-types   shared domain structs
```

**Inbound email**
```
Internet → TCP :25 → SMTP state machine (EHLO/STARTTLS/MAIL/RCPT/DATA)
  → MIME parse (extract text/plain, decode quoted-printable)
  → store in PostgreSQL
```

**Outbound email**
```
POST /mailboxes/:addr/send
  → IP rate limit (5/day) + mailbox rate limit (5/day, VPN-resistant)
  → build RFC 2822 message → DKIM sign (RSA-SHA256)
  → SMTP relay (Resend, port 587, STARTTLS + AUTH LOGIN)
```

---

## HTTP API

```bash
# Create mailbox
curl -X POST https://phantom-mail.sanjula.online/mailboxes

# Fetch emails
curl https://phantom-mail.sanjula.online/mailboxes/user%40mail.phantom-mail.sanjula.online/emails

# Send email
curl -X POST .../mailboxes/user%40.../send \
  -H "Content-Type: application/json" \
  -d '{"to":"someone@gmail.com","subject":"Hello","body":"Hi"}'
```

---

## Self-hosting on AWS Free Tier

| Step | Detail |
|---|---|
| Instance | EC2 t2.micro, Ubuntu 22.04 |
| Domain | A record `mail.your-domain.com` → Elastic IP |
| MX | `your-domain.com MX 10 mail.your-domain.com` |
| SPF | `v=spf1 mx ~all` |
| DKIM | RSA key pair + TXT `mail._domainkey.your-domain.com` |
| DMARC | `v=DMARC1; p=quarantine` |
| TLS | Let's Encrypt via Certbot |
| Outbound | Resend free tier (3,000 emails/month) |
| Frontend | Vercel free tier |

Monthly cost: **$0**

---

## Testing

```bash
cargo test   # 12 tests, no database needed
```

---

## Contact

Built by **Sanjula** · [sanjula.online](https://www.sanjula.online) · [sanjula692@gmail.com](mailto:sanjula692@gmail.com)

[Email & Privacy Policy](https://phantom-mail.sanjula.online/policy)
