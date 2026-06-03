use async_trait::async_trait;
use reqwest::header::{HeaderMap, ETAG};
use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ArchiveDownload, FileBlob, FileEntry, Page, PageOpts, ProviderCapabilities, Result,
    SkillSourceProvider, SourceRef, Workspace,
};
use std::path::Path;

use crate::archive::GiteeArchiveDownload;
use crate::models::{RepoResponse, TreeResponse};
use crate::permissions::gitee_capabilities;
use crate::provider::GiteeProvider;
use crate::util::{url_encode, validate_repo_path};

#[async_trait]
impl SkillSourceProvider for GiteeProvider {
    fn id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        gitee_capabilities()
    }

    async fn list_sources(&self, opts: PageOpts) -> Result<Page<Workspace>> {
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let raw: Page<RepoResponse> = self
            .get_page_json(&format!(
                "/user/repos?sort=updated&direction=desc&per_page={per_page}&page={}",
                url_encode(&page)
            ))
            .await?;
        Ok(Page {
            items: raw
                .items
                .into_iter()
                .map(|repo| repo.into_workspace(&self.instance_id))
                .collect(),
            next_cursor: raw.next_cursor,
        })
    }

    async fn get_source(&self, reference: &WorkspaceRef) -> Result<Workspace> {
        let (owner, repo) = Self::owner_repo(reference);
        let raw: RepoResponse = self.get_json(&format!("/repos/{owner}/{repo}")).await?;
        Ok(raw.into_workspace(&self.instance_id))
    }

    async fn list_files(&self, reference: &WorkspaceRef, at: &SourceRef) -> Result<Vec<FileEntry>> {
        let (owner, repo) = Self::owner_repo(reference);
        let ref_name = self.source_ref_value(reference, at).await?;
        let raw: TreeResponse = self
            .get_json(&format!(
                "/repos/{owner}/{repo}/git/trees/{}?recursive=1",
                url_encode(&ref_name)
            ))
            .await?;
        Ok(raw.tree.into_iter().map(FileEntry::from).collect())
    }

    async fn read_file(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
        path: &str,
    ) -> Result<FileBlob> {
        validate_repo_path(path)?;
        let (owner, repo) = Self::owner_repo(reference);
        let ref_name = self.source_ref_value(reference, at).await?;
        let (headers, bytes) = self
            .get_bytes(&format!(
                "/repos/{owner}/{repo}/raw/{}?ref={}",
                url_encode(path.trim_matches('/')),
                url_encode(&ref_name)
            ))
            .await?;
        Ok(FileBlob {
            path: path.to_owned(),
            sha: blob_sha(&headers),
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
        let ref_name = self.source_ref_value(reference, at).await?;
        let archive: GiteeArchiveDownload = self
            .download_tarball_with_progress(reference, &ref_name, destination, |done, total| {
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

fn blob_sha(headers: &HeaderMap) -> String {
    headers
        .get("x-gitee-blob-id")
        .or_else(|| headers.get("x-gitee-content-sha256"))
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}
