"""OTP / verification-code extraction from email subject + body."""

import re
from typing import Optional

# Ordered most-specific → least-specific so we return the most likely code.
_PATTERNS = [
    # "Your verification code is 482910" / "Code: 482910"
    re.compile(
        r'(?:verification|confirm(?:ation)?|security|one.?time|otp|auth(?:entication)?'
        r'|access|login|sign.?in|activat(?:e|ion)|reset|temporary)'
        r'(?:\s+(?:code|pin|password|token|key|number|otp))?'
        r'\s*(?:is|:|-|–)?\s*'
        r'([A-Z0-9]{4,8})',
        re.IGNORECASE,
    ),
    # Bold/bracketed standalone code: **482910** or [482910]
    re.compile(r'(?:\*\*|\[)(\d{4,8})(?:\*\*|\])'),
    # Six-digit standalone number (most common OTP length)
    re.compile(r'\b(\d{6})\b'),
    # Eight-digit
    re.compile(r'\b(\d{8})\b'),
    # Four-digit PIN
    re.compile(r'\b(\d{4})\b'),
    # Alphanumeric token e.g. "AB1234" or "1A2B3C"
    re.compile(r'\b([A-Z]{2}\d{4,6}|[A-Z0-9]{6,8})\b'),
]


def extract_otp(subject: str, body: str) -> Optional[str]:
    text = f"{subject}\n{body}"
    for pattern in _PATTERNS:
        m = pattern.search(text)
        if m:
            return m.group(1)
    return None
