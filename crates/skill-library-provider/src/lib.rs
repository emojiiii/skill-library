use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use skill_library_core::WorkspaceRef;

pub type Result<T> = std::result::Result<T, ProviderError>;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("not found: {resource}")]
    NotFound {
        resource: String,
        reference: Option<String>,
    },
    #[error("forbidden: {resource}")]
    Forbidden {
        resource: String,
        reason: Option<String>,
    },
    #[error("unauthorized: {reason:?}")]
    Unauthorized {
        reason: UnauthorizedReason,
        missing_scopes: Vec<String>,
    },
    #[error("rate limited: retry after {retry_after_ms}ms")]
    RateLimited {
        retry_after_ms: u64,
        bucket: RateLimitBucket,
    },
    #[error("network error: {cause}")]
    NetworkError { cause: String },
    #[error("provider unavailable: {message}")]
    ProviderUnavailable {
        status: Option<u16>,
        message: String,
    },
    #[error("conflict: {resource}")]
    Conflict {
        resource: String,
        hint: Option<String>,
    },
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnauthorizedReason {
    TokenInvalid,
    TokenExpired,
    ScopeMissing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitBucket {
    Core,
    Graphql,
    Search,
    Secondary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub webhook: bool,
    pub release_assets: bool,
    pub graphql: bool,
    pub device_flow: bool,
    pub refresh_token: bool,
    pub bot_identity: bool,
    pub pull_requests: bool,
    pub invitations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

impl<T> Page<T> {
    pub fn single(items: Vec<T>) -> Self {
        Self {
            items,
            next_cursor: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub provider: String,
    pub owner: String,
    pub repo: String,
    pub full_name: String,
    pub default_branch: String,
    pub visibility: String,
    pub permission: PermissionLevel,
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub kind: FileKind,
    pub sha: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    Submodule,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileBlob {
    pub path: String,
    pub sha: String,
    pub bytes: Vec<u8>,
    pub encoding: String,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub sha: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Release {
    pub id: String,
    pub tag_name: String,
    pub name: Option<String>,
    pub prerelease: bool,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Commit {
    pub sha: String,
    pub message: String,
    pub authored_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefComparison {
    pub status: String,
    pub ahead_by: u32,
    pub behind_by: u32,
    pub files: Vec<ChangedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangedFile {
    pub filename: String,
    pub status: String,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    Admin,
    Maintain,
    Write,
    Triage,
    Read,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Member {
    pub login: String,
    pub role: PermissionLevel,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub html_url: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Invitation {
    pub id: String,
    pub login_or_email: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebhookHandle {
    pub id: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebhookConfig {
    pub events: Vec<String>,
    pub callback_url: String,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvitationInput {
    pub login_or_email: String,
    pub role: PermissionLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestInput {
    pub branch_name: String,
    pub title: String,
    pub body: String,
    pub base: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GitRef {
    Branch(String),
    Tag(String),
    Sha(String),
}

impl GitRef {
    pub fn value(&self) -> &str {
        match self {
            Self::Branch(value) | Self::Tag(value) | Self::Sha(value) => value,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageOpts {
    pub cursor: Option<String>,
    pub per_page: Option<u16>,
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
