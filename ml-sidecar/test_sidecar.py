"""Quick smoke tests for the ML sidecar — run with: pytest test_sidecar.py"""

import pytest
from extractor import extract_otp
from classifier import classify_category, score_spam


# ── OTP extraction ─────────────────────────────────────────────────────────────

def test_extracts_six_digit_code():
    assert extract_otp("Your code", "Your verification code is 482910. Do not share it.") == "482910"

def test_extracts_labelled_code():
    assert extract_otp("Confirm your email", "Use code: 7731 to verify your account.") == "7731"

def test_extracts_bold_code():
    assert extract_otp("Login code", "Your one-time code is **294817**") == "294817"

def test_no_code_returns_none():
    assert extract_otp("Welcome!", "Thanks for signing up. Enjoy the service.") is None


# ── Category classification ────────────────────────────────────────────────────

def test_category_verification():
    assert classify_category("Verify your email", "Click to confirm your account.") == "verification"

def test_category_newsletter():
    assert classify_category("Weekly digest", "Unsubscribe from this newsletter.") == "newsletter"

def test_category_receipt():
    assert classify_category("Your order #1234", "Payment received. Invoice attached.") == "receipt"

def test_category_other():
    assert classify_category("Hey", "Just wanted to say hi!") == "other"


# ── Spam scoring ───────────────────────────────────────────────────────────────

def test_clean_email_low_spam_score():
    assert score_spam("Verify your GitHub account", "Click the link to confirm.", "noreply@github.com") < 0.2

def test_spam_email_high_score():
    score = score_spam(
        "YOU WON A PRIZE!!!",
        "Claim your FREE cash bonus NOW! Limited time offer. Act now! Click here now!",
        "promo@spam.example",
    )
    assert score >= 0.4
