use flate2::read::GzDecoder;
use skill_library_provider::{PermissionLevel, ProviderError, Result, WebhookConfig};
use std::path::{Component, Path, PathBuf};

use crate::models::{CreateWebhookConfig, CreateWebhookRequest};

pub(crate) fn urlencoding_simple(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

pub(crate) fn extract_tarball(bytes: &[u8], destination: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(destination)
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    let mut archive = tar::Archive::new(GzDecoder::new(bytes));
    let mut top_level: Option<PathBuf> = None;
    for entry in archive
        .entries()
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?
    {
        let mut entry = entry.map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
        let path = entry
            .path()
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?
            .to_path_buf();
        validate_archive_path(&path)?;
        let entry_type = entry.header().entry_type();
        let is_pax_header = entry_type.is_pax_global_extensions()
            || entry_type.is_pax_local_extensions()
            || path.as_os_str() == "pax_global_header";
        if top_level.is_none() && !is_pax_header {
            top_level = path.components().next().map(|component| {
                let mut root = PathBuf::new();
                root.push(component.as_os_str());
                root
            });
        }
        entry
            .unpack_in(destination)
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    }
    Ok(top_level
        .map(|root| destination.join(root))
        .unwrap_or_else(|| destination.to_path_buf()))
}

fn validate_archive_path(path: &Path) -> Result<()> {
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

pub(crate) fn validate_branch_ref(branch: &str) -> Result<()> {
    if branch.trim().is_empty()
        || branch.starts_with('/')
        || branch.ends_with('/')
        || branch.contains("..")
        || branch.contains('\\')
        || branch.contains(' ')
        || branch.contains("refs/")
    {
        return Err(ProviderError::Conflict {
            resource: "branch".to_owned(),
            hint: Some(format!("unsafe branch ref: {branch}")),
        });
    }
    Ok(())
}

pub(crate) fn github_permission_role(role: &PermissionLevel) -> Result<&'static str> {
    match role {
        PermissionLevel::Admin => Ok("admin"),
        PermissionLevel::Maintain => Ok("maintain"),
        PermissionLevel::Write => Ok("push"),
        PermissionLevel::Triage => Ok("triage"),
        PermissionLevel::Read => Ok("pull"),
        PermissionLevel::None => Err(ProviderError::InvalidResponse(
            "invitation role cannot be none".to_owned(),
        )),
    }
}

pub(crate) fn github_webhook_request(config: WebhookConfig) -> Result<CreateWebhookRequest> {
    if config.callback_url.trim().is_empty() {
        return Err(ProviderError::InvalidResponse(
            "webhook callback url is required".to_owned(),
        ));
    }
    if config.secret.trim().is_empty() {
        return Err(ProviderError::InvalidResponse(
            "webhook secret is required".to_owned(),
        ));
    }
    let mut events = Vec::new();
    for event in config.events {
        let event = event.trim();
        if !event.is_empty() && !events.iter().any(|existing| existing == event) {
            events.push(event.to_owned());
        }
    }
    if events.is_empty() {
        events.push("push".to_owned());
    }

    Ok(CreateWebhookRequest {
        name: "web",
        active: true,
        events,
        config: CreateWebhookConfig {
            url: config.callback_url,
            content_type: "json",
            secret: config.secret,
            insecure_ssl: "0",
        },
    })
}
