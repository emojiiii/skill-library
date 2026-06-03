use reqwest::header::{HeaderMap, CONTENT_LENGTH};
use skill_library_provider::{ProviderError, Result};
use std::path::{Component, Path};

pub(crate) fn snippet(value: &str) -> String {
    value.chars().take(200).collect()
}

pub(crate) fn redact_access_token(path: &str) -> String {
    const MARKER: &str = "access_token=";
    let mut rest = path;
    let mut out = String::with_capacity(path.len());
    while let Some(index) = rest.find(MARKER) {
        let (head, tail) = rest.split_at(index);
        out.push_str(head);
        out.push_str(MARKER);
        out.push_str("[REDACTED]");
        let value_start = MARKER.len();
        let value_tail = &tail[value_start..];
        match value_tail.find('&') {
            Some(next) => {
                rest = &value_tail[next..];
            }
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
}

pub(crate) fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

pub(crate) fn validate_repo_path(path: &str) -> Result<()> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() {
        return Err(ProviderError::InvalidResponse(
            "repo path cannot be empty".to_owned(),
        ));
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(ProviderError::InvalidResponse(format!(
                "repo path is unsafe: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_archive_path(path: &Path) -> Result<()> {
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(ProviderError::InvalidResponse(format!(
                "archive path is unsafe: {}",
                path.display()
            )));
        }
    }
    Ok(())
}
