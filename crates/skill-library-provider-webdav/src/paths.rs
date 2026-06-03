use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use skill_library_provider::{ProviderError, Result};
use std::path::{Component, Path};

const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');

pub(crate) fn normalize_repo_path_lossy(value: &str) -> String {
    value
        .replace('\\', "/")
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn normalize_url_path(value: &str) -> String {
    let mut normalized = format!("/{}", normalize_repo_path_lossy(value));
    if normalized != "/" && value.ends_with('/') {
        normalized.push('/');
    }
    normalized.trim_end_matches('/').to_owned()
}

pub(crate) fn join_repo_path(left: &str, right: &str) -> String {
    let left = normalize_repo_path_lossy(left);
    let right = normalize_repo_path_lossy(right);
    match (left.is_empty(), right.is_empty()) {
        (true, true) => String::new(),
        (true, false) => right,
        (false, true) => left,
        (false, false) => format!("{left}/{right}"),
    }
}

pub(crate) fn collection_request_path(path: &str) -> String {
    let path = normalize_repo_path_lossy(path);
    if path.is_empty() {
        String::new()
    } else {
        format!("{path}/")
    }
}

pub(crate) fn encode_path(path: &str) -> String {
    normalize_repo_path_lossy(path)
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| utf8_percent_encode(segment, PATH_SEGMENT_ENCODE_SET).to_string())
        .collect::<Vec<_>>()
        .join("/")
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
