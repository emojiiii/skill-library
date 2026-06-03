use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub graphql: Capability,
    pub device_flow: Capability,
    pub oauth_loopback: Capability,
    pub personal_access_token: Capability,
    pub repository_archive: Capability,
    pub release_assets: Capability,
    pub change_requests: Capability,
    pub direct_file_write: Capability,
    pub invitations: Capability,
    pub members: Capability,
    pub webhooks: Capability,
    pub discussions: Capability,
    pub file_storage: Capability,
    pub versions_index: Capability,
}

impl ProviderCapabilities {
    pub fn github() -> Self {
        Self {
            graphql: Capability::Supported,
            device_flow: Capability::Supported,
            oauth_loopback: Capability::Unsupported,
            personal_access_token: Capability::Supported,
            repository_archive: Capability::Supported,
            release_assets: Capability::Supported,
            change_requests: Capability::Supported,
            direct_file_write: Capability::Supported,
            invitations: Capability::Supported,
            members: Capability::Supported,
            webhooks: Capability::Supported,
            discussions: Capability::Supported,
            file_storage: Capability::Unsupported,
            versions_index: Capability::Unsupported,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Supported,
    Unsupported,
    RequiresConfig,
    Experimental,
}
