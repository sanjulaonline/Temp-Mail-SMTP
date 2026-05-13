use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlMeta {
    /// Extracted OTP / verification code, if any.
    pub otp_code: Option<String>,
    /// Spam probability 0.0–1.0.
    pub spam_score: f32,
    /// verification | newsletter | notification | receipt | other
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub id: String,
    pub sender: String,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub timestamp: DateTime<Utc>,
    /// Populated asynchronously by the ML sidecar after the email is stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ml_meta: Option<MlMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporaryMailbox {
    pub email_address: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}
