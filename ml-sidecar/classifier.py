"""Spam scoring and email category classification."""

import re
from typing import Literal

Category = Literal["verification", "newsletter", "notification", "receipt", "other"]

_VERIFICATION = {
    "verify", "verification", "confirm", "confirmation", "otp", "one-time",
    "one time", "code", "pin", "activate", "activation", "authenticate",
    "authentication", "token", "security code", "login code", "sign in",
    "signin", "your code", "access code", "reset password", "password reset",
}
_NEWSLETTER = {
    "unsubscribe", "newsletter", "mailing list", "subscribe", "weekly",
    "digest", "edition", "issue", "curated", "roundup",
}
_NOTIFICATION = {
    "notification", "alert", "reminder", "update", "notice", "activity",
    "new message", "missed", "flagged", "mention",
}
_RECEIPT = {
    "order", "receipt", "invoice", "payment", "purchase", "transaction",
    "billing", "refund", "shipment", "shipped", "delivered", "tracking",
    "your order", "order confirmation",
}

_SPAM_PHRASES = [
    "winner", "you won", "you have won", "prize", "free gift", "claim now",
    "act now", "limited time offer", "earn money", "make money", "cash bonus",
    "guaranteed", "risk-free", "risk free", "no cost", "100% free",
    "click here now", "buy now", "order now", "special promotion",
    "dear friend", "dear beneficiary", "bank transfer", "wire transfer",
    "bitcoin", "crypto investment", "investment opportunity", "double your",
    "nigerian", "lottery", "jackpot",
]
_SPAM_RE = re.compile(
    r'(?:' + '|'.join(re.escape(p) for p in _SPAM_PHRASES) + r')',
    re.IGNORECASE,
)


def classify_category(subject: str, body: str) -> Category:
    text = (subject + " " + body).lower()
    scores: dict[str, int] = {
        "verification": sum(1 for kw in _VERIFICATION if kw in text),
        "newsletter":   sum(1 for kw in _NEWSLETTER   if kw in text),
        "notification": sum(1 for kw in _NOTIFICATION if kw in text),
        "receipt":      sum(1 for kw in _RECEIPT      if kw in text),
    }
    best, top = max(scores.items(), key=lambda x: x[1])
    return best if top > 0 else "other"  # type: ignore[return-value]


def score_spam(subject: str, body: str, sender: str) -> float:
    text = subject + " " + body
    score = 0.0

    # Known spam phrases
    hits = len(_SPAM_RE.findall(text))
    score += min(hits * 0.20, 0.60)

    # Excessive exclamation marks
    if text.count("!") > 3:
        score += 0.10

    # Dollar signs
    if text.count("$") > 2:
        score += 0.10

    # ALL-CAPS words in subject (≥ 3 chars)
    caps = sum(1 for w in subject.split() if w.isupper() and len(w) >= 3)
    score += min(caps * 0.05, 0.15)

    # Suspicious sender domains (free webmail used for spam)
    _THROWAWAY = {"mailinator.com", "guerrillamail.com", "tempmail.com"}
    domain = sender.split("@")[-1].lower() if "@" in sender else ""
    if domain in _THROWAWAY:
        score += 0.20

    return round(min(score, 1.0), 3)
