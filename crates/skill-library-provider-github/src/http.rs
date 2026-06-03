use serde::Deserialize;
use skill_library_provider::{ProviderError, Result, UnauthorizedReason};

pub(crate) async fn map_response<T: for<'de> Deserialize<'de>>(
    path: &str,
    method: &str,
    response: reqwest::Response,
) -> Result<T> {
    let status = response.status();
    if status.is_success() {
        let bytes = response
            .bytes()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        return serde_json::from_slice::<T>(&bytes).map_err(|err| {
            let body = String::from_utf8_lossy(&bytes);
            let snippet: String = body.chars().take(200).collect();
            tracing::error!(
                target: "skill-library-github",
                method,
                path,
                status = status.as_u16(),
                body = %snippet,
                error = %err,
                "deserialize failed"
            );
            ProviderError::InvalidResponse(format!(
                "{method} {path} ({status}): deserialize failed: {err} - body: {snippet}"
            ))
        });
    }

    let message = response.text().await.unwrap_or_else(|_| status.to_string());
    let snippet: String = message.chars().take(200).collect();
    tracing::warn!(
        target: "skill-library-github",
        method,
        path,
        status = status.as_u16(),
        body = %snippet,
        "non-success response"
    );
    Err(provider_error_from_status(
        status,
        format!("{method} {path} ({status}): {snippet}"),
    ))
}

pub(crate) async fn map_empty_response(
    path: &str,
    method: &str,
    response: reqwest::Response,
) -> Result<()> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }

    let message = response.text().await.unwrap_or_else(|_| status.to_string());
    let snippet: String = message.chars().take(200).collect();
    tracing::warn!(
        target: "skill-library-github",
        method,
        path,
        status = status.as_u16(),
        body = %snippet,
        "non-success response"
    );
    Err(provider_error_from_status(
        status,
        format!("{method} {path} ({status}): {snippet}"),
    ))
}

pub(crate) fn provider_error_from_status(
    status: reqwest::StatusCode,
    message: String,
) -> ProviderError {
    match status.as_u16() {
        401 => ProviderError::Unauthorized {
            reason: UnauthorizedReason::TokenInvalid,
            missing_scopes: Vec::new(),
        },
        403 if message.to_ascii_lowercase().contains("rate limit") => ProviderError::RateLimited {
            retry_after_ms: 60_000,
            bucket: skill_library_provider::RateLimitBucket::Core,
        },
        429 => ProviderError::RateLimited {
            retry_after_ms: 60_000,
            bucket: skill_library_provider::RateLimitBucket::Core,
        },
        403 => ProviderError::Forbidden {
            resource: "github".to_owned(),
            reason: Some(message),
        },
        404 => ProviderError::NotFound {
            resource: message,
            reference: None,
        },
        409 | 422 => ProviderError::Conflict {
            resource: "github resource".to_owned(),
            hint: Some(message),
        },
        status if status >= 500 => ProviderError::ProviderUnavailable {
            status: Some(status),
            message,
        },
        _ => ProviderError::InvalidResponse(message),
    }
}

pub(crate) fn parse_scope_header(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(str::to_owned)
        .collect()
}
