use async_trait::async_trait;
use flate2::read::GzDecoder;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use skill_library_core::WorkspaceRef;
use skill_library_provider::{ArchiveDownload, ArchiveProvider, ProviderError, Result};
use std::path::{Path, PathBuf};

use crate::http::provider_error_from_status;
use crate::provider::GitLabProvider;
use crate::util::{content_length, snippet, url_encode, validate_archive_path};

#[derive(Debug, Clone)]
pub struct GitLabArchiveDownload {
    pub ref_name: String,
    pub sha256: String,
    pub bytes: u64,
    pub extracted_root: PathBuf,
}

impl GitLabProvider {
    pub async fn download_archive_with_progress<F>(
        &self,
        reference: &WorkspaceRef,
        ref_name: &str,
        destination: impl AsRef<Path>,
        mut on_progress: F,
    ) -> Result<GitLabArchiveDownload>
    where
        F: FnMut(u64, Option<u64>) + Send,
    {
        let project = Self::project_id(reference);
        let path = format!(
            "/projects/{project}/repository/archive.tar.gz?sha={}",
            url_encode(ref_name)
        );
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-gitlab", method = "GET", path);
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| status.to_string());
            let body = snippet(&body);
            tracing::warn!(
                target: "skill-library-gitlab",
                method = "GET",
                path,
                status = status.as_u16(),
                body = %body,
                "non-success response"
            );
            return Err(provider_error_from_status(
                status,
                format!("GET {path} ({status}): {body}"),
            ));
        }

        let total = response
            .content_length()
            .or_else(|| content_length(response.headers()));
        let mut downloaded = 0_u64;
        let mut hasher = Sha256::new();
        let mut bytes = Vec::with_capacity(total.unwrap_or(0) as usize);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
            downloaded += chunk.len() as u64;
            hasher.update(&chunk);
            bytes.extend_from_slice(&chunk);
            on_progress(downloaded, total);
        }

        let extracted_root = extract_tarball(&bytes, destination.as_ref())?;
        Ok(GitLabArchiveDownload {
            ref_name: ref_name.to_owned(),
            sha256: format!("{:x}", hasher.finalize()),
            bytes: downloaded,
            extracted_root,
        })
    }
}

#[async_trait]
impl ArchiveProvider for GitLabProvider {
    async fn download_archive(
        &self,
        reference: &WorkspaceRef,
        ref_name: &str,
        destination: &Path,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<ArchiveDownload> {
        let archive = self
            .download_archive_with_progress(reference, ref_name, destination, |done, total| {
                on_progress(done, total);
            })
            .await?;
        Ok(ArchiveDownload {
            destination: destination.to_path_buf(),
            extracted_root: archive.extracted_root,
            ref_name: archive.ref_name,
            sha256: Some(archive.sha256),
            bytes: Some(archive.bytes),
        })
    }
}

fn extract_tarball(bytes: &[u8], destination: &Path) -> Result<PathBuf> {
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
