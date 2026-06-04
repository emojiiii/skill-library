use skill_library_provider::{Capability, PermissionLevel, ProviderCapabilities};

use crate::models::ProjectPermissions;

pub(crate) fn gitlab_capabilities() -> ProviderCapabilities {
    ProviderCapabilities {
        graphql: Capability::Unsupported,
        device_flow: Capability::Unsupported,
        oauth_loopback: Capability::Unsupported,
        personal_access_token: Capability::Supported,
        repository_archive: Capability::Supported,
        release_assets: Capability::Supported,
        change_requests: Capability::Experimental,
        direct_file_write: Capability::Unsupported,
        invitations: Capability::Supported,
        members: Capability::Supported,
        webhooks: Capability::Unsupported,
        discussions: Capability::Unsupported,
        file_storage: Capability::Unsupported,
        versions_index: Capability::Unsupported,
    }
}

pub(crate) fn split_project_path(
    path_with_namespace: &str,
    fallback_path: &str,
) -> (String, String) {
    match path_with_namespace.rsplit_once('/') {
        Some((owner, repo)) => (owner.to_owned(), repo.to_owned()),
        None => (String::new(), fallback_path.to_owned()),
    }
}

pub(crate) fn permission_from_project(
    permissions: Option<&ProjectPermissions>,
    visibility: &str,
) -> PermissionLevel {
    let access = permissions
        .and_then(|permissions| {
            [
                permissions.project_access.as_ref(),
                permissions.group_access.as_ref(),
            ]
            .into_iter()
            .flatten()
            .map(|access| access.access_level)
            .max()
        })
        .unwrap_or(0);
    let permission = permission_from_access_level(access);
    if matches!(permission, PermissionLevel::None) && visibility == "public" {
        PermissionLevel::Read
    } else {
        permission
    }
}

pub(crate) fn permission_from_access_level(access_level: u32) -> PermissionLevel {
    match access_level {
        50.. => PermissionLevel::Admin,
        40..=49 => PermissionLevel::Maintain,
        30..=39 => PermissionLevel::Write,
        20..=29 | 10..=19 => PermissionLevel::Read,
        _ => PermissionLevel::None,
    }
}
