"""Phantom Mail — ML sidecar service.

Exposes POST /analyse which accepts an email and returns:
  - otp_code:   extracted verification code (or null)
  - spam_score: 0.0–1.0 probability of spam
  - category:   verification | newsletter | notification | receipt | other
"""

from typing import Optional

from fastapi import FastAPI
from pydantic import BaseModel

from classifier import classify_category, score_spam
from extractor import extract_otp

app = FastAPI(title="Phantom Mail ML Sidecar", version="1.0.0")


class EmailRequest(BaseModel):
    id: str
    sender: str
    subject: str
    body: str


class AnalysisResult(BaseModel):
    otp_code: Optional[str]
    spam_score: float
    category: str


@app.get("/health")
def health() -> dict:
    return {"status": "ok"}


@app.post("/analyse", response_model=AnalysisResult)
async def analyse(req: EmailRequest) -> AnalysisResult:
    return AnalysisResult(
        otp_code=extract_otp(req.subject, req.body),
        spam_score=score_spam(req.subject, req.body, req.sender),
        category=classify_category(req.subject, req.body),
    )
