use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceRef {
    Latest,
    Version(String),
    Git(GitRef),
    Revision(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveDownload {
    pub destination: PathBuf,
    pub extracted_root: PathBuf,
    pub ref_name: String,
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeRequestInput {
    pub branch_name: String,
    pub title: String,
    pub body: String,
    pub base: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeRequest {
    pub number: u64,
    pub title: String,
    pub html_url: String,
    pub state: String,
    pub provider_kind: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageOpts {
    pub cursor: Option<String>,
    pub per_page: Option<u16>,
}
