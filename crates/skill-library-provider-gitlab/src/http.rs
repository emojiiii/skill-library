use serde::Deserialize;
use skill_library_provider::{ProviderError, RateLimitBucket, Result, UnauthorizedReason};

use crate::util::snippet;

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
            let body = snippet(&body);
            tracing::error!(
                target: "skill-library-gitlab",
                method,
                path,
                status = status.as_u16(),
                body = %body,
                error = %err,
                "deserialize failed"
            );
            ProviderError::InvalidResponse(format!(
                "{method} {path} ({status}): deserialize failed: {err} - body: {body}"
            ))
        });
    }

    let message = response.text().await.unwrap_or_else(|_| status.to_string());
    let body = snippet(&message);
    let detail = response_error_message(method, path, status, &message);
    tracing::warn!(
        target: "skill-library-gitlab",
        method,
        path,
        status = status.as_u16(),
        body = %body,
        "non-success response"
    );
    Err(provider_error_from_status(status, detail))
}

pub(crate) fn response_error_message(
    method: &str,
    path: &str,
    status: reqwest::StatusCode,
    body: &str,
) -> String {
    let fallback = || format!("{method} {path} ({status}): {}", snippet(body));
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return fallback();
    };
    let error = value.get("error").and_then(|value| value.as_str());
    let description = value
        .get("error_description")
        .and_then(|value| value.as_str())
        .or_else(|| value.get("message").and_then(|value| value.as_str()));
    let scope = value.get("scope").and_then(|value| value.as_str());
    if error.is_none() && description.is_none() && scope.is_none() {
        return fallback();
    }
    let mut parts = Vec::new();
    if let Some(error) = error {
        parts.push(error.to_owned());
    }
    if let Some(description) = description {
        parts.push(description.to_owned());
    }
    if let Some(scope) = scope {
        parts.push(format!("required scope: {scope}"));
    }
    format!("{method} {path} ({status}): {}", parts.join(" - "))
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
        403 => ProviderError::Forbidden {
            resource: "gitlab".to_owned(),
            reason: Some(message),
        },
        404 => ProviderError::NotFound {
            resource: message,
            reference: None,
        },
        409 => ProviderError::Conflict {
            resource: "gitlab resource".to_owned(),
            hint: Some(message),
        },
        429 => ProviderError::RateLimited {
            retry_after_ms: 60_000,
            bucket: RateLimitBucket::Core,
        },
        status if status >= 500 => ProviderError::ProviderUnavailable {
            status: Some(status),
            message,
        },
        _ => ProviderError::InvalidResponse(message),
    }
}
