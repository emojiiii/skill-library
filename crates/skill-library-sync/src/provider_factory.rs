use crate::{remote_scan::SkillCatalogProvider, Result, SyncError};
use skill_library_core::{
    default_provider_instances, normalize_provider_id, ProviderCredential, ProviderInstance,
    ProviderKind, WorkspaceRef,
};
use skill_library_provider::{GitRepositoryProvider, PublishProvider, SkillSourceProvider};
use skill_library_provider_gitee::GiteeProvider;
use skill_library_provider_github::GitHubProvider;
use skill_library_provider_gitlab::GitLabProvider;
use skill_library_provider_webdav::WebDavProvider;
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct ProviderHandles {
    #[allow(dead_code)]
    pub source: Arc<dyn SkillSourceProvider>,
    #[allow(dead_code)]
    pub git: Option<Arc<dyn GitRepositoryProvider>>,
    #[allow(dead_code)]
    pub publish: Option<Arc<dyn PublishProvider>>,
    pub catalog: Option<Arc<dyn SkillCatalogProvider>>,
}

pub struct ProviderFactory {
    instances: BTreeMap<String, ProviderInstance>,
}

impl Default for ProviderFactory {
    fn default() -> Self {
        Self::new(default_provider_instances())
    }
}

impl ProviderFactory {
    pub fn new(instances: impl IntoIterator<Item = ProviderInstance>) -> Self {
        Self {
            instances: instances
                .into_iter()
                .map(|instance| (normalize_provider_id(&instance.id), instance))
                .collect(),
        }
    }

    pub fn from_config_path(path: impl AsRef<std::path::Path>) -> crate::Result<Self> {
        Ok(Self::new(
            skill_library_core::provider_instances_from_config_path(path)?,
        ))
    }

    pub fn build(
        &self,
        workspace: &WorkspaceRef,
        credential: Option<&ProviderCredential>,
    ) -> Result<ProviderHandles> {
        let provider_id = workspace.normalized_provider();
        let instance = self
            .instances
            .get(&provider_id)
            .ok_or_else(|| SyncError::ProviderUnsupported(provider_id.clone()))?;
        match instance.kind {
            ProviderKind::GitHub => {
                let provider = Arc::new(self.build_github_provider(workspace, credential)?);
                Ok(ProviderHandles {
                    source: provider.clone(),
                    git: Some(provider.clone()),
                    publish: Some(provider.clone()),
                    catalog: Some(provider),
                })
            }
            ProviderKind::GitLab => {
                let provider = Arc::new(self.build_gitlab_provider(workspace, credential)?);
                Ok(ProviderHandles {
                    source: provider.clone(),
                    git: Some(provider.clone()),
                    publish: None,
                    catalog: Some(provider),
                })
            }
            ProviderKind::Gitee => {
                let provider = Arc::new(self.build_gitee_provider(workspace, credential)?);
                Ok(ProviderHandles {
                    source: provider.clone(),
                    git: Some(provider.clone()),
                    publish: None,
                    catalog: Some(provider),
                })
            }
            ProviderKind::WebDav => {
                let provider = Arc::new(self.build_webdav_provider(workspace, credential)?);
                Ok(ProviderHandles {
                    source: provider.clone(),
                    git: None,
                    publish: None,
                    catalog: Some(provider),
                })
            }
            _ => Err(SyncError::ProviderUnsupported(provider_id)),
        }
    }

    pub fn build_github_provider(
        &self,
        workspace: &WorkspaceRef,
        credential: Option<&ProviderCredential>,
    ) -> Result<GitHubProvider> {
        let provider_id = workspace.normalized_provider();
        let instance = self
            .instances
            .get(&provider_id)
            .ok_or_else(|| SyncError::ProviderUnsupported(provider_id.clone()))?;
        if !matches!(instance.kind, ProviderKind::GitHub) {
            return Err(SyncError::ProviderUnsupported(provider_id));
        }
        let token = credential
            .filter(|credential| {
                normalize_provider_id(&credential.metadata.provider) == provider_id
            })
            .map(|credential| credential.token.clone())
            .filter(|token| !token.trim().is_empty());
        GitHubProvider::for_instance(instance, token).map_err(provider_to_sync_error)
    }

    pub fn build_gitlab_provider(
        &self,
        workspace: &WorkspaceRef,
        credential: Option<&ProviderCredential>,
    ) -> Result<GitLabProvider> {
        let provider_id = workspace.normalized_provider();
        let instance = self
            .instances
            .get(&provider_id)
            .ok_or_else(|| SyncError::ProviderUnsupported(provider_id.clone()))?;
        if !matches!(instance.kind, ProviderKind::GitLab) {
            return Err(SyncError::ProviderUnsupported(provider_id));
        }
        let token = credential
            .filter(|credential| {
                normalize_provider_id(&credential.metadata.provider) == provider_id
            })
            .map(|credential| credential.token.clone())
            .filter(|token| !token.trim().is_empty());
        GitLabProvider::for_instance(instance, token).map_err(provider_to_sync_error)
    }

    pub fn build_gitee_provider(
        &self,
        workspace: &WorkspaceRef,
        credential: Option<&ProviderCredential>,
    ) -> Result<GiteeProvider> {
        let provider_id = workspace.normalized_provider();
        let instance = self
            .instances
            .get(&provider_id)
            .ok_or_else(|| SyncError::ProviderUnsupported(provider_id.clone()))?;
        if !matches!(instance.kind, ProviderKind::Gitee) {
            return Err(SyncError::ProviderUnsupported(provider_id));
        }
        let token = credential
            .filter(|credential| {
                normalize_provider_id(&credential.metadata.provider) == provider_id
            })
            .map(|credential| credential.token.clone())
            .filter(|token| !token.trim().is_empty());
        GiteeProvider::for_instance(instance, token).map_err(provider_to_sync_error)
    }

    pub fn build_webdav_provider(
        &self,
        workspace: &WorkspaceRef,
        credential: Option<&ProviderCredential>,
    ) -> Result<WebDavProvider> {
        let provider_id = workspace.normalized_provider();
        let instance = self
            .instances
            .get(&provider_id)
            .ok_or_else(|| SyncError::ProviderUnsupported(provider_id.clone()))?;
        if !matches!(instance.kind, ProviderKind::WebDav) {
            return Err(SyncError::ProviderUnsupported(provider_id));
        }
        let credential = credential.filter(|credential| {
            normalize_provider_id(&credential.metadata.provider) == provider_id
        });
        WebDavProvider::for_instance(instance, credential).map_err(provider_to_sync_error)
    }
}

fn provider_to_sync_error(err: skill_library_provider::ProviderError) -> SyncError {
    SyncError::Io(std::io::Error::new(
        std::io::ErrorKind::Other,
        err.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use skill_library_provider::Capability;

    fn workspace(provider: &str) -> WorkspaceRef {
        WorkspaceRef {
            provider: provider.to_owned(),
            owner: "group/subgroup".to_owned(),
            repo: "team-skills".to_owned(),
            remote_id: None,
        }
    }

    #[test]
    fn factory_builds_github_publish_provider_handle() {
        let handles = ProviderFactory::default()
            .build(&workspace("github.com"), None)
            .unwrap();
        let caps = handles.source.capabilities();

        assert_eq!(handles.source.id(), "github.com");
        assert!(handles.git.is_some());
        assert!(handles.publish.is_some());
        assert!(handles.catalog.is_some());
        assert_eq!(caps.change_requests, Capability::Supported);
        assert_eq!(caps.discussions, Capability::Supported);
        assert_eq!(caps.repository_archive, Capability::Supported);
    }

    #[test]
    fn factory_builds_gitlab_provider_handles() {
        let handles = ProviderFactory::default()
            .build(&workspace("gitlab.com"), None)
            .unwrap();
        let caps = handles.source.capabilities();

        assert_eq!(handles.source.id(), "gitlab.com");
        assert!(handles.git.is_some());
        assert!(handles.publish.is_none());
        assert!(handles.catalog.is_some());
        assert_eq!(caps.change_requests, Capability::Experimental);
        assert_eq!(caps.discussions, Capability::Unsupported);
        assert_eq!(caps.repository_archive, Capability::Supported);
    }

    #[test]
    fn factory_builds_gitee_provider_handles() {
        let handles = ProviderFactory::default()
            .build(&workspace("gitee.com"), None)
            .unwrap();
        let caps = handles.source.capabilities();

        assert_eq!(handles.source.id(), "gitee.com");
        assert!(handles.git.is_some());
        assert!(handles.publish.is_none());
        assert!(handles.catalog.is_some());
        assert_eq!(caps.change_requests, Capability::Experimental);
        assert_eq!(caps.discussions, Capability::Unsupported);
        assert_eq!(caps.repository_archive, Capability::Supported);
    }

    #[test]
    fn factory_builds_webdav_provider_without_git_handle() {
        let instance = ProviderInstance {
            id: "webdav.test".to_owned(),
            kind: ProviderKind::WebDav,
            display_name: "Test WebDAV".to_owned(),
            web_base_url: "https://dav.example.test/skills".to_owned(),
            api_base_url: "https://dav.example.test/skills".to_owned(),
            auth_modes: vec![skill_library_core::AuthMode::Basic],
            enabled: true,
        };
        let handles = ProviderFactory::new([instance])
            .build(&workspace("webdav.test"), None)
            .unwrap();
        let caps = handles.source.capabilities();

        assert_eq!(handles.source.id(), "webdav.test");
        assert!(handles.git.is_none());
        assert!(handles.publish.is_none());
        assert!(handles.catalog.is_some());
        assert_eq!(caps.file_storage, Capability::Supported);
        assert_eq!(caps.versions_index, Capability::Supported);
        assert_eq!(caps.change_requests, Capability::Unsupported);
        assert_eq!(caps.discussions, Capability::Unsupported);
    }
}
