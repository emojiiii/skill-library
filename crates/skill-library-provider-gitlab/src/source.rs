use async_trait::async_trait;
use reqwest::header::{HeaderMap, ETAG};
use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ArchiveDownload, FileBlob, FileEntry, Page, PageOpts, ProviderCapabilities, Result,
    SkillSourceProvider, SourceRef, Workspace,
};
use std::path::Path;

use crate::archive::GitLabArchiveDownload;
use crate::models::{ProjectResponse, TreeEntryResponse};
use crate::permissions::gitlab_capabilities;
use crate::provider::GitLabProvider;
use crate::util::{url_encode, validate_repo_path};

#[async_trait]
impl SkillSourceProvider for GitLabProvider {
    fn id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        gitlab_capabilities()
    }

    async fn list_sources(&self, opts: PageOpts) -> Result<Page<Workspace>> {
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let raw: Page<ProjectResponse> = self
            .get_page_json(&format!(
                "/projects?membership=true&simple=true&order_by=last_activity_at&sort=desc&per_page={per_page}&page={}",
                url_encode(&page)
            ))
            .await?;
        Ok(Page {
            items: raw
                .items
                .into_iter()
                .map(|project| project.into_workspace(&self.instance_id))
                .collect(),
            next_cursor: raw.next_cursor,
        })
    }

    async fn get_source(&self, reference: &WorkspaceRef) -> Result<Workspace> {
        let project = Self::project_id(reference);
        let raw: ProjectResponse = self.get_json(&format!("/projects/{project}")).await?;
        Ok(raw.into_workspace(&self.instance_id))
    }

    async fn list_files(&self, reference: &WorkspaceRef, at: &SourceRef) -> Result<Vec<FileEntry>> {
        let project = Self::project_id(reference);
        let ref_name = self.source_ref_value(reference, at).await?;
        let mut page = "1".to_owned();
        let mut entries = Vec::new();
        loop {
            let raw: Page<TreeEntryResponse> = self
                .get_page_json(&format!(
                    "/projects/{project}/repository/tree?recursive=true&ref={}&per_page=100&page={}",
                    url_encode(&ref_name),
                    url_encode(&page)
                ))
                .await?;
            entries.extend(raw.items.into_iter().map(FileEntry::from));
            match raw.next_cursor {
                Some(next) => page = next,
                None => break,
            }
        }
        Ok(entries)
    }

    async fn read_file(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
        path: &str,
    ) -> Result<FileBlob> {
        validate_repo_path(path)?;
        let project = Self::project_id(reference);
        let ref_name = self.source_ref_value(reference, at).await?;
        let file_path = url_encode(path.trim_matches('/'));
        let (headers, bytes) = self
            .get_bytes(&format!(
                "/projects/{project}/repository/files/{file_path}/raw?ref={}",
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
        let archive: GitLabArchiveDownload = self
            .download_archive_with_progress(reference, &ref_name, destination, |done, total| {
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
        .get("x-gitlab-blob-id")
        .or_else(|| headers.get("x-gitlab-content-sha256"))
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}
