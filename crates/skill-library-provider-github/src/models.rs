use serde::{Deserialize, Serialize};
use skill_library_provider::{Member, PermissionLevel, PullRequest, Workspace};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitHubUser {
    pub login: String,
    pub id: u64,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitHubTokenInfo {
    pub user: GitHubUser,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitHubArchiveDownload {
    pub ref_name: String,
    pub sha256: String,
    pub bytes: u64,
    pub extracted_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPublishInput {
    pub branch_name: String,
    pub commit_message: String,
    pub title: String,
    pub body: String,
    pub base: Option<String>,
    pub files: Vec<GitHubPublishFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPublishFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPublishResult {
    pub pull_request: PullRequest,
    pub uploaded: Vec<GitHubUploadedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUploadedFile {
    pub path: String,
    pub sha: String,
}

#[derive(Debug, Clone, Copy)]
pub enum PullRequestQueryState {
    Open,
    Closed,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestSummary {
    pub number: u64,
    pub title: String,
    pub html_url: String,
    pub state: String,
    pub draft: bool,
    pub merged: bool,
    pub author: Option<String>,
    pub head_ref: String,
    pub base_ref: String,
    pub head_repo: Option<String>,
    pub base_repo: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueComment {
    pub id: u64,
    pub html_url: String,
    pub body: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PullRequestListItemResponse {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) html_url: String,
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) draft: bool,
    #[serde(default)]
    pub(crate) merged_at: Option<String>,
    pub(crate) user: Option<OwnerResponse>,
    pub(crate) head: PullRequestRefResponse,
    pub(crate) base: PullRequestRefResponse,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    #[serde(default)]
    pub(crate) body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PullRequestRefResponse {
    #[serde(default, rename = "ref")]
    pub(crate) ref_name: String,
    #[serde(default)]
    pub(crate) repo: Option<PullRequestRefRepoResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PullRequestRefRepoResponse {
    pub(crate) full_name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PullRequestFileResponse {
    pub(crate) filename: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) patch: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ClosePullRequestRequest {
    pub(crate) state: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct IssueCommentRequest<'a> {
    pub(crate) body: &'a str,
}

impl From<PullRequestListItemResponse> for PullRequestSummary {
    fn from(value: PullRequestListItemResponse) -> Self {
        let merged = value.merged_at.is_some();
        Self {
            number: value.number,
            title: value.title,
            html_url: value.html_url,
            state: value.state,
            draft: value.draft,
            merged,
            author: value.user.map(|user| user.login),
            head_ref: value.head.ref_name,
            base_ref: value.base.ref_name,
            head_repo: value.head.repo.map(|repo| repo.full_name),
            base_repo: value.base.repo.map(|repo| repo.full_name),
            created_at: value.created_at,
            updated_at: value.updated_at,
            body: value.body,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryEvent {
    pub id: String,
    pub event_type: String,
    pub actor: Option<String>,
    pub created_at: String,
    pub summary: String,
    pub html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RepositoryEventResponse {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) event_type: Option<String>,
    pub(crate) actor: Option<OwnerResponse>,
    pub(crate) created_at: String,
    #[serde(default)]
    pub(crate) payload: Option<serde_json::Value>,
}

impl From<RepositoryEventResponse> for RepositoryEvent {
    fn from(value: RepositoryEventResponse) -> Self {
        let event_type = value
            .event_type
            .clone()
            .unwrap_or_else(|| "unknown".to_owned());
        let (summary, html_url) = match (event_type.as_str(), value.payload.as_ref()) {
            ("PushEvent", Some(payload)) => {
                let r#ref = payload
                    .get("ref")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                let count = payload
                    .get("commits")
                    .and_then(|v| v.as_array())
                    .map(|c| c.len())
                    .unwrap_or(0);
                (format!("Pushed {count} commit(s) to {ref}"), None)
            }
            ("PullRequestEvent", Some(payload)) => {
                let action = payload.get("action").and_then(|v| v.as_str()).unwrap_or("");
                let pr = payload.get("pull_request");
                let title = pr
                    .and_then(|p| p.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let url = pr
                    .and_then(|p| p.get("html_url"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                (format!("Pull request {action}: {title}"), url)
            }
            ("ReleaseEvent", Some(payload)) => {
                let release = payload.get("release");
                let tag = release
                    .and_then(|r| r.get("tag_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let url = release
                    .and_then(|r| r.get("html_url"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                (format!("Released {tag}"), url)
            }
            ("CreateEvent", Some(payload)) => {
                let kind = payload
                    .get("ref_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let r#ref = payload.get("ref").and_then(|v| v.as_str()).unwrap_or("");
                (format!("Created {kind} {ref}"), None)
            }
            (other, _) => (other.to_owned(), None),
        };
        Self {
            id: value.id,
            event_type,
            actor: value.actor.map(|user| user.login),
            created_at: value.created_at,
            summary,
            html_url,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInvitation {
    pub id: u64,
    pub repository_full_name: String,
    pub inviter: Option<String>,
    pub permissions: String,
    pub html_url: String,
    pub created_at: String,
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author: Option<String>,
    pub author_email: Option<String>,
    pub authored_at: String,
    pub html_url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommitListItemResponse {
    pub(crate) sha: String,
    pub(crate) html_url: String,
    pub(crate) commit: CommitInnerResponse,
    #[serde(default)]
    pub(crate) author: Option<OwnerResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommitInnerResponse {
    pub(crate) message: String,
    pub(crate) author: CommitAuthorResponse,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommitAuthorResponse {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) email: Option<String>,
    pub(crate) date: String,
}

impl From<CommitListItemResponse> for CommitSummary {
    fn from(value: CommitListItemResponse) -> Self {
        let short_sha = value.sha.chars().take(7).collect();
        let message = value.message_first_line();
        Self {
            sha: value.sha,
            short_sha,
            message,
            author: value
                .author
                .map(|user| user.login)
                .or_else(|| value.commit.author.name.clone()),
            author_email: value.commit.author.email.clone(),
            authored_at: value.commit.author.date,
            html_url: value.html_url,
        }
    }
}

impl CommitListItemResponse {
    fn message_first_line(&self) -> String {
        self.commit
            .message
            .lines()
            .next()
            .unwrap_or(&self.commit.message)
            .to_owned()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct RepositoryInvitationResponse {
    pub(crate) id: u64,
    pub(crate) repository: RepositoryInvitationRepoResponse,
    pub(crate) inviter: Option<OwnerResponse>,
    pub(crate) permissions: String,
    pub(crate) html_url: String,
    pub(crate) created_at: String,
    #[serde(default)]
    pub(crate) expired: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RepositoryInvitationRepoResponse {
    pub(crate) full_name: String,
}

impl From<RepositoryInvitationResponse> for RepositoryInvitation {
    fn from(value: RepositoryInvitationResponse) -> Self {
        Self {
            id: value.id,
            repository_full_name: value.repository.full_name,
            inviter: value.inviter.map(|user| user.login),
            permissions: value.permissions,
            html_url: value.html_url,
            created_at: value.created_at,
            expired: value.expired,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct DeviceCodeRequest<'a> {
    pub(crate) client_id: &'a str,
    pub(crate) scope: &'a str,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct DeviceTokenRequest<'a> {
    pub(crate) client_id: &'a str,
    pub(crate) device_code: &'a str,
    pub(crate) grant_type: &'a str,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceTokenResponse {
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateWebhookRequest {
    pub(crate) name: &'static str,
    pub(crate) active: bool,
    pub(crate) events: Vec<String>,
    pub(crate) config: CreateWebhookConfig,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateWebhookConfig {
    pub(crate) url: String,
    pub(crate) content_type: &'static str,
    pub(crate) secret: String,
    pub(crate) insecure_ssl: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateGitRefRequest {
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    pub(crate) sha: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GitReferenceResponse {
    pub(crate) object: GitReferenceObject,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GitReferenceObject {
    pub(crate) sha: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct PutContentRequest {
    pub(crate) message: String,
    pub(crate) content: String,
    pub(crate) branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sha: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PutContentResponse {
    pub(crate) content: PutContentInfo,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PutContentInfo {
    pub(crate) sha: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreatePullRequestRequest {
    pub(crate) title: String,
    pub(crate) head: String,
    pub(crate) base: String,
    pub(crate) body: String,
    pub(crate) draft: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PullRequestResponse {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) html_url: String,
    pub(crate) state: String,
}

impl From<PullRequestResponse> for PullRequest {
    fn from(value: PullRequestResponse) -> Self {
        Self {
            number: value.number,
            title: value.title,
            html_url: value.html_url,
            state: value.state,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct MergePullRequestRequest {
    pub(crate) merge_method: &'static str,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MergePullRequestResponse {
    pub(crate) merged: bool,
    #[serde(default)]
    pub(crate) message: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CollaboratorInvitationRequest {
    pub(crate) permission: &'static str,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CollaboratorInvitationResponse {
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) state: Option<String>,
    #[serde(default)]
    pub(crate) invitee: Option<InviteeResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct InviteeResponse {
    pub(crate) login: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WebhookResponse {
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) config: Option<WebhookResponseConfig>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WebhookResponseConfig {
    #[serde(default)]
    pub(crate) url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RepoResponse {
    pub(crate) name: String,
    pub(crate) full_name: String,
    pub(crate) default_branch: String,
    pub(crate) private: bool,
    pub(crate) html_url: Option<String>,
    pub(crate) owner: OwnerResponse,
    pub(crate) permissions: Option<RepoPermissions>,
}

impl From<RepoResponse> for Workspace {
    fn from(repo: RepoResponse) -> Self {
        let permission = repo
            .permissions
            .map(PermissionLevel::from)
            .unwrap_or(PermissionLevel::None);
        Workspace {
            provider: "github.com".to_owned(),
            owner: repo.owner.login,
            repo: repo.name,
            full_name: repo.full_name,
            default_branch: repo.default_branch,
            visibility: if repo.private { "private" } else { "public" }.to_owned(),
            permission,
            html_url: repo.html_url,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct OwnerResponse {
    pub(crate) login: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RepoPermissions {
    pub(crate) admin: bool,
    #[serde(default)]
    pub(crate) maintain: bool,
    pub(crate) push: bool,
    #[serde(default)]
    pub(crate) triage: bool,
    pub(crate) pull: bool,
}

impl From<RepoPermissions> for PermissionLevel {
    fn from(permissions: RepoPermissions) -> Self {
        if permissions.admin {
            PermissionLevel::Admin
        } else if permissions.maintain {
            PermissionLevel::Maintain
        } else if permissions.push {
            PermissionLevel::Write
        } else if permissions.triage {
            PermissionLevel::Triage
        } else if permissions.pull {
            PermissionLevel::Read
        } else {
            PermissionLevel::None
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CollaboratorResponse {
    pub(crate) login: String,
    #[serde(default)]
    pub(crate) avatar_url: Option<String>,
    #[serde(default)]
    pub(crate) permissions: Option<RepoPermissions>,
}

impl From<CollaboratorResponse> for Member {
    fn from(value: CollaboratorResponse) -> Self {
        Self {
            login: value.login,
            role: value
                .permissions
                .map(PermissionLevel::from)
                .unwrap_or(PermissionLevel::None),
            avatar_url: value.avatar_url,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct TreeResponse {
    pub(crate) tree: Vec<TreeEntryResponse>,
    #[serde(default)]
    pub(crate) truncated: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TreeEntryResponse {
    pub(crate) path: String,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) sha: Option<String>,
    pub(crate) size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ContentResponse {
    pub(crate) path: String,
    pub(crate) sha: String,
    pub(crate) content: String,
    pub(crate) encoding: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TagResponse {
    pub(crate) name: String,
    pub(crate) commit: TagCommitResponse,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TagCommitResponse {
    pub(crate) sha: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReleaseResponse {
    pub(crate) id: u64,
    pub(crate) tag_name: String,
    pub(crate) name: Option<String>,
    pub(crate) prerelease: bool,
    pub(crate) body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompareResponse {
    pub(crate) status: String,
    pub(crate) ahead_by: u32,
    pub(crate) behind_by: u32,
    pub(crate) files: Option<Vec<CompareFileResponse>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompareFileResponse {
    pub(crate) filename: String,
    pub(crate) status: String,
    pub(crate) patch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PermissionResponse {
    pub(crate) permission: String,
}
