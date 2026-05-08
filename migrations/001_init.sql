-- Phantom Mail — initial schema
-- Migration: 001_init.sql

CREATE TABLE IF NOT EXISTS mailboxes (
    email_address TEXT PRIMARY KEY,
    created_at    TIMESTAMPTZ NOT NULL,
    expires_at    TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS emails (
    id          TEXT        PRIMARY KEY,
    sender      TEXT        NOT NULL,
    recipient   TEXT        NOT NULL REFERENCES mailboxes(email_address) ON DELETE CASCADE,
    subject     TEXT        NOT NULL,
    body        TEXT        NOT NULL,
    timestamp   TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_emails_recipient ON emails (recipient);
CREATE INDEX IF NOT EXISTS idx_mailboxes_expires ON mailboxes (expires_at);
