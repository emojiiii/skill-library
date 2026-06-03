mod auth;
mod capabilities;
mod client;
mod download;
mod error;
mod index;
mod paths;
mod propfind;

#[cfg(test)]
mod tests;

use async_trait::async_trait;
use reqwest::header::{ETAG, LAST_MODIFIED};
use reqwest::Url;
use sha2::{Digest, Sha256};
use skill_library_core::{ProviderCredential, ProviderInstance, ProviderKind, WorkspaceRef};
use skill_library_provider::{
    ArchiveDownload, FileBlob, FileEntry, Page, PageOpts, PermissionLevel, ProviderCapabilities,
    ProviderError, Result, SkillSourceProvider, SourceRef, Workspace,
};
use std::path::Path;

pub use auth::WebDavAuth;
use capabilities::webdav_capabilities;
pub use index::{WebDavIndex, WebDavIndexSkill};
use paths::{join_repo_path, normalize_repo_path_lossy, validate_repo_path};

pub struct WebDavProvider {
    pub(crate) client: reqwest::Client,
    pub(crate) api_base: Url,
    pub(crate) instance_id: String,
    pub(crate) auth: Option<WebDavAuth>,
}

impl WebDavProvider {
    pub fn anonymous(api_base: impl AsRef<str>) -> Result<Self> {
        Self::with_instance_base_url("webdav", api_base.as_ref(), None)
    }

    pub fn for_instance(
        instance: &ProviderInstance,
        credential: Option<&ProviderCredential>,
    ) -> Result<Self> {
        if !matches!(instance.kind, ProviderKind::WebDav) {
            return Err(ProviderError::InvalidResponse(format!(
                "provider instance {} is not a WebDAV provider",
                instance.id
            )));
        }
        Self::with_instance_base_url(
            instance.id.clone(),
            instance.api_base_url.clone(),
            credential.and_then(WebDavAuth::from_credential),
        )
    }

    pub async fn read_index(&self, reference: &WorkspaceRef) -> Result<Option<WebDavIndex>> {
        match self
            .read_file(reference, &SourceRef::Latest, ".skill-library/index.json")
            .await
        {
            Ok(blob) => serde_json::from_slice::<WebDavIndex>(&blob.bytes)
                .map(Some)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string())),
            Err(ProviderError::NotFound { .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub(crate) fn workspace_path(reference: &WorkspaceRef) -> String {
        reference
            .remote_id
            .as_deref()
            .map(normalize_repo_path_lossy)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                let owner = normalize_repo_path_lossy(&reference.owner);
                let repo = normalize_repo_path_lossy(&reference.repo);
                match (owner.is_empty(), repo.is_empty()) {
                    (true, true) => String::new(),
                    (true, false) => repo,
                    (false, true) => owner,
                    (false, false) => format!("{owner}/{repo}"),
                }
            })
    }

    fn collection_path(&self, reference: &WorkspaceRef, at: &SourceRef) -> Result<String> {
        let root = Self::workspace_path(reference);
        match at {
            SourceRef::Latest | SourceRef::Git(_) => Ok(root),
            SourceRef::Version(version) | SourceRef::Revision(version) => {
                let version = version.trim();
                if version.is_empty() || version == "latest" {
                    Ok(root)
                } else {
                    Ok(join_repo_path(&root, version))
                }
            }
        }
    }
}

#[async_trait]
impl SkillSourceProvider for WebDavProvider {
    fn id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        webdav_capabilities()
    }

    async fn list_sources(&self, _opts: PageOpts) -> Result<Page<Workspace>> {
        let entries = self.propfind_collection("", "1").await?;
        let mut workspaces = Vec::new();
        for entry in entries {
            if !entry.is_collection || entry.relative_path.is_empty() {
                continue;
            }
            let path = normalize_repo_path_lossy(&entry.relative_path);
            if path.contains('/') {
                continue;
            }
            workspaces.push(webdav_workspace(&self.instance_id, &path, None));
        }
        workspaces.sort_by(|a, b| a.full_name.cmp(&b.full_name));
        Ok(Page::single(workspaces))
    }

    async fn get_source(&self, reference: &WorkspaceRef) -> Result<Workspace> {
        let path = Self::workspace_path(reference);
        let entries = self.propfind_collection(&path, "0").await?;
        let exists = entries
            .iter()
            .any(|entry| entry.relative_path.is_empty() && entry.is_collection);
        if !exists {
            return Err(ProviderError::NotFound {
                resource: format!("WebDAV collection '{}'", reference.full_name()),
                reference: Some(path),
            });
        }
        let html_url = self.url_for(&path).ok().map(|url| url.to_string());
        Ok(webdav_workspace(&self.instance_id, &path, html_url))
    }

    async fn list_files(&self, reference: &WorkspaceRef, at: &SourceRef) -> Result<Vec<FileEntry>> {
        let root_path = self.collection_path(reference, at)?;
        self.list_collection_files(&root_path).await
    }

    async fn read_file(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
        path: &str,
    ) -> Result<FileBlob> {
        validate_repo_path(path)?;
        let root_path = self.collection_path(reference, at)?;
        let remote_path = join_repo_path(&root_path, path);
        let (headers, bytes) = self.get_bytes(&remote_path).await?;
        let sha = headers
            .get(ETAG)
            .or_else(|| headers.get(LAST_MODIFIED))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                format!("sha256:{:x}", hasher.finalize())
            });
        Ok(FileBlob {
            path: path.to_owned(),
            sha,
            bytes,
            encoding: "raw".to_owned(),
            etag: headers
                .get(ETAG)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
        })
    }

    async fn download_snapshot(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
        destination: &Path,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<ArchiveDownload> {
        let ref_name = match at {
            SourceRef::Latest | SourceRef::Git(_) => "latest".to_owned(),
            SourceRef::Version(version) | SourceRef::Revision(version) => {
                let value = version.trim();
                if value.is_empty() {
                    "latest".to_owned()
                } else {
                    value.to_owned()
                }
            }
        };
        let extracted_root = destination.join("webdav-snapshot");
        if extracted_root.exists() {
            std::fs::remove_dir_all(&extracted_root)
                .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
        }
        std::fs::create_dir_all(&extracted_root)
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;

        let mut hasher = Sha256::new();
        let mut downloaded = 0_u64;
        if let Some(index) = self.read_index(reference).await? {
            let version = if ref_name == "latest" {
                None
            } else {
                Some(ref_name.as_str())
            };
            self.download_indexed_snapshot(
                reference,
                &index,
                version,
                &extracted_root,
                &mut hasher,
                &mut downloaded,
                on_progress,
            )
            .await?;
        } else if ref_name == "latest" {
            let root_path = Self::workspace_path(reference);
            self.download_collection_into(
                &root_path,
                &extracted_root,
                &mut hasher,
                &mut downloaded,
                on_progress,
            )
            .await?;
        } else {
            return Err(ProviderError::NotFound {
                resource: format!("WebDAV version '{ref_name}'"),
                reference: Some(reference.full_name()),
            });
        }

        Ok(ArchiveDownload {
            destination: destination.to_path_buf(),
            extracted_root,
            ref_name,
            sha256: Some(format!("{:x}", hasher.finalize())),
            bytes: Some(downloaded),
        })
    }
}

fn webdav_workspace(provider: &str, path: &str, html_url: Option<String>) -> Workspace {
    let path = normalize_repo_path_lossy(path);
    let (owner, repo) = match path.rsplit_once('/') {
        Some((owner, repo)) => (owner.to_owned(), repo.to_owned()),
        None => (String::new(), path.clone()),
    };
    Workspace {
        provider: provider.to_owned(),
        owner,
        repo,
        full_name: path,
        default_branch: "latest".to_owned(),
        visibility: "private".to_owned(),
        permission: PermissionLevel::Read,
        html_url,
    }
}
