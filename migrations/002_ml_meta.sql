-- Phantom Mail — ML metadata column
-- Migration: 002_ml_meta.sql
-- Stores JSON produced by the ML sidecar (otp_code, spam_score, category).
-- NULL until the sidecar processes the email.

ALTER TABLE emails ADD COLUMN IF NOT EXISTS ml_meta TEXT;
