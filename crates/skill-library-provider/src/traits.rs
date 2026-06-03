use async_trait::async_trait;
use skill_library_core::WorkspaceRef;
use std::path::Path;

use crate::{
    ArchiveDownload, Capability, ChangeRequest, ChangeRequestInput, FileBlob, FileEntry, GitRef,
    Invitation, InvitationInput, Member, Page, PageOpts, PermissionLevel, ProviderCapabilities,
    PullRequest, PullRequestInput, RefComparison, Release, Result, SourceRef, Tag, WebhookConfig,
    WebhookHandle, Workspace,
};

#[async_trait]
pub trait SkillSourceProvider: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;

    async fn list_sources(&self, opts: PageOpts) -> Result<Page<Workspace>>;
    async fn get_source(&self, reference: &WorkspaceRef) -> Result<Workspace>;
    async fn list_files(&self, reference: &WorkspaceRef, at: &SourceRef) -> Result<Vec<FileEntry>>;
    async fn read_file(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
        path: &str,
    ) -> Result<FileBlob>;
    async fn download_snapshot(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
        destination: &Path,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<ArchiveDownload>;
}

#[async_trait]
pub trait GitRepositoryProvider: SkillSourceProvider {
    async fn list_tags(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Tag>>;
    async fn list_releases(
        &self,
        reference: &WorkspaceRef,
        opts: PageOpts,
    ) -> Result<Page<Release>>;
    async fn compare_refs(
        &self,
        reference: &WorkspaceRef,
        base: &GitRef,
        head: &GitRef,
    ) -> Result<RefComparison>;
    async fn check_permission(
        &self,
        reference: &WorkspaceRef,
        login: &str,
    ) -> Result<PermissionLevel>;
}

#[async_trait]
pub trait ArchiveProvider: Send + Sync {
    async fn download_archive(
        &self,
        reference: &WorkspaceRef,
        ref_name: &str,
        destination: &Path,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> Result<ArchiveDownload>;
}

#[async_trait]
pub trait PublishProvider: Send + Sync {
    async fn create_change_request(
        &self,
        reference: &WorkspaceRef,
        input: ChangeRequestInput,
    ) -> Result<ChangeRequest>;
}

pub trait SocialProvider: Send + Sync {
    fn social_capability(&self) -> Capability {
        Capability::Unsupported
    }
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;

    async fn list_workspaces(&self, opts: PageOpts) -> Result<Page<Workspace>>;
    async fn get_workspace(&self, reference: &WorkspaceRef) -> Result<Workspace>;
    async fn list_files(&self, reference: &WorkspaceRef, at: &GitRef) -> Result<Vec<FileEntry>>;
    async fn read_file(
        &self,
        reference: &WorkspaceRef,
        at: &GitRef,
        path: &str,
    ) -> Result<FileBlob>;
    async fn list_tags(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Tag>>;
    async fn list_releases(
        &self,
        reference: &WorkspaceRef,
        opts: PageOpts,
    ) -> Result<Page<Release>>;
    async fn compare_refs(
        &self,
        reference: &WorkspaceRef,
        base: &GitRef,
        head: &GitRef,
    ) -> Result<RefComparison>;
    async fn list_members(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Member>>;
    async fn register_webhook(
        &self,
        reference: &WorkspaceRef,
        config: WebhookConfig,
    ) -> Result<WebhookHandle>;
    async fn create_invitation(
        &self,
        reference: &WorkspaceRef,
        invite: InvitationInput,
    ) -> Result<Invitation>;
    async fn create_pull_request(
        &self,
        reference: &WorkspaceRef,
        input: PullRequestInput,
    ) -> Result<PullRequest>;
    async fn merge_pull_request(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<PullRequest>;
    async fn check_permission(
        &self,
        reference: &WorkspaceRef,
        login: &str,
    ) -> Result<PermissionLevel>;
}
