use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, ProviderError>;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("not found: {resource}")]
    NotFound {
        resource: String,
        reference: Option<String>,
    },
    #[error("forbidden: {resource}")]
    Forbidden {
        resource: String,
        reason: Option<String>,
    },
    #[error("unauthorized: {reason:?}")]
    Unauthorized {
        reason: UnauthorizedReason,
        missing_scopes: Vec<String>,
    },
    #[error("rate limited: retry after {retry_after_ms}ms")]
    RateLimited {
        retry_after_ms: u64,
        bucket: RateLimitBucket,
    },
    #[error("network error: {cause}")]
    NetworkError { cause: String },
    #[error("provider unavailable: {message}")]
    ProviderUnavailable {
        status: Option<u16>,
        message: String,
    },
    #[error("conflict: {resource}")]
    Conflict {
        resource: String,
        hint: Option<String>,
    },
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnauthorizedReason {
    TokenInvalid,
    TokenExpired,
    ScopeMissing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitBucket {
    Core,
    Graphql,
    Search,
    Secondary,
}
