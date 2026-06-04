use skill_library_provider::{Capability, PermissionLevel, ProviderCapabilities};

use crate::models::RepoPermissions;

pub(crate) fn gitee_capabilities() -> ProviderCapabilities {
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

pub(crate) fn split_repo_path(full_name: &str, fallback_path: &str) -> (String, String) {
    match full_name.rsplit_once('/') {
        Some((owner, repo)) => (owner.to_owned(), repo.to_owned()),
        None => (String::new(), fallback_path.to_owned()),
    }
}

pub(crate) fn permission_from_repo(
    permissions: Option<&RepoPermissions>,
    visibility: &str,
) -> PermissionLevel {
    match permissions {
        Some(permissions) if permissions.admin => PermissionLevel::Admin,
        Some(permissions) if permissions.push => PermissionLevel::Write,
        Some(permissions) if permissions.pull => PermissionLevel::Read,
        _ if visibility == "public" => PermissionLevel::Read,
        _ => PermissionLevel::None,
    }
}

pub(crate) fn permission_from_name(value: &str) -> PermissionLevel {
    match value {
        "admin" => PermissionLevel::Admin,
        "push" | "write" => PermissionLevel::Write,
        "pull" | "read" => PermissionLevel::Read,
        _ => PermissionLevel::None,
    }
}
