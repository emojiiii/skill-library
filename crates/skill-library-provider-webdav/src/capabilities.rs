use skill_library_provider::{Capability, ProviderCapabilities};

pub(crate) fn webdav_capabilities() -> ProviderCapabilities {
    ProviderCapabilities {
        graphql: Capability::Unsupported,
        device_flow: Capability::Unsupported,
        oauth_loopback: Capability::Unsupported,
        personal_access_token: Capability::RequiresConfig,
        repository_archive: Capability::Unsupported,
        release_assets: Capability::Unsupported,
        change_requests: Capability::Unsupported,
        direct_file_write: Capability::Unsupported,
        invitations: Capability::Unsupported,
        members: Capability::Unsupported,
        webhooks: Capability::Unsupported,
        discussions: Capability::Unsupported,
        file_storage: Capability::Supported,
        versions_index: Capability::Supported,
    }
}
