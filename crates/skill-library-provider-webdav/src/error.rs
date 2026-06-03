use reqwest::StatusCode;
use skill_library_provider::{ProviderError, RateLimitBucket, UnauthorizedReason};

pub(crate) fn provider_error_from_status(status: StatusCode, message: String) -> ProviderError {
    match status {
        StatusCode::UNAUTHORIZED => ProviderError::Unauthorized {
            reason: UnauthorizedReason::TokenInvalid,
            missing_scopes: Vec::new(),
        },
        StatusCode::FORBIDDEN => ProviderError::Forbidden {
            resource: message,
            reason: None,
        },
        StatusCode::NOT_FOUND => ProviderError::NotFound {
            resource: message,
            reference: None,
        },
        StatusCode::CONFLICT => ProviderError::Conflict {
            resource: message,
            hint: None,
        },
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimited {
            retry_after_ms: 0,
            bucket: RateLimitBucket::Core,
        },
        _ => ProviderError::ProviderUnavailable {
            status: Some(status.as_u16()),
            message,
        },
    }
}

pub(crate) fn snippet(value: &str) -> String {
    const MAX: usize = 240;
    let trimmed = value.trim();
    let snippet = trimmed.chars().take(MAX).collect::<String>();
    if snippet.len() < trimmed.len() {
        format!("{snippet}...")
    } else {
        snippet
    }
}
