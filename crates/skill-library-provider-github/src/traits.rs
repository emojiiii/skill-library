use async_trait::async_trait;
use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ArchiveDownload, ArchiveProvider, ChangeRequest, ChangeRequestInput, FileBlob, FileEntry,
    GitRef, GitRepositoryProvider, Page, PageOpts, PermissionLevel, Provider, ProviderCapabilities,
    PublishProvider, PullRequestInput, RefComparison, Release, Result, SkillSourceProvider,
    SourceRef, Tag, Workspace,
};
use std::path::Path;

use crate::GitHubProvider;

impl GitHubProvider {
    pub(crate) async fn git_ref_for_source_ref(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
    ) -> Result<GitRef> {
        Ok(match at {
            SourceRef::Latest => {
                let workspace = <Self as Provider>::get_workspace(self, reference).await?;
                GitRef::Branch(workspace.default_branch)
            }
            SourceRef::Version(version) => GitRef::Tag(version.clone()),
            SourceRef::Git(git_ref) => git_ref.clone(),
            SourceRef::Revision(revision) => GitRef::Sha(revision.clone()),
        })
    }
}

#[async_trait]
impl SkillSourceProvider for GitHubProvider {
    fn id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::github()
    }

    async fn list_sources(&self, opts: PageOpts) -> Result<Page<Workspace>> {
        <Self as Provider>::list_workspaces(self, opts).await
    }

    async fn get_source(&self, reference: &WorkspaceRef) -> Result<Workspace> {
        <Self as Provider>::get_workspace(self, reference).await
    }

    async fn list_files(&self, reference: &WorkspaceRef, at: &SourceRef) -> Result<Vec<FileEntry>> {
        let git_ref = self.git_ref_for_source_ref(reference, at).await?;
        <Self as Provider>::list_files(self, reference, &git_ref).await
    }

    async fn read_file(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
        path: &str,
    ) -> Result<FileBlob> {
        let git_ref = self.git_ref_for_source_ref(reference, at).await?;
        <Self as Provider>::read_file(self, reference, &git_ref, path).await
    }

    async fn download_snapshot(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
        destination: &Path,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<ArchiveDownload> {
        let git_ref = self.git_ref_for_source_ref(reference, at).await?;
        let archive = self
            .download_tarball_with_progress(
                reference,
                git_ref.value(),
                destination,
                |done, total| {
                    on_progress(done, total);
                },
            )
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

#[async_trait]
impl GitRepositoryProvider for GitHubProvider {
    async fn list_tags(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Tag>> {
        <Self as Provider>::list_tags(self, reference, opts).await
    }

    async fn list_releases(
        &self,
        reference: &WorkspaceRef,
        opts: PageOpts,
    ) -> Result<Page<Release>> {
        <Self as Provider>::list_releases(self, reference, opts).await
    }

    async fn compare_refs(
        &self,
        reference: &WorkspaceRef,
        base: &GitRef,
        head: &GitRef,
    ) -> Result<RefComparison> {
        <Self as Provider>::compare_refs(self, reference, base, head).await
    }

    async fn check_permission(
        &self,
        reference: &WorkspaceRef,
        login: &str,
    ) -> Result<PermissionLevel> {
        <Self as Provider>::check_permission(self, reference, login).await
    }
}

#[async_trait]
impl ArchiveProvider for GitHubProvider {
    async fn download_archive(
        &self,
        reference: &WorkspaceRef,
        ref_name: &str,
        destination: &Path,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<ArchiveDownload> {
        let archive = self
            .download_tarball_with_progress(reference, ref_name, destination, |done, total| {
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

#[async_trait]
impl PublishProvider for GitHubProvider {
    async fn create_change_request(
        &self,
        reference: &WorkspaceRef,
        input: ChangeRequestInput,
    ) -> Result<ChangeRequest> {
        let pr = <Self as Provider>::create_pull_request(
            self,
            reference,
            PullRequestInput {
                branch_name: input.branch_name,
                title: input.title,
                body: input.body,
                base: input.base,
            },
        )
        .await?;
        Ok(ChangeRequest {
            number: pr.number,
            title: pr.title,
            html_url: pr.html_url,
            state: pr.state,
            provider_kind: "pull_request".to_owned(),
        })
    }
}
