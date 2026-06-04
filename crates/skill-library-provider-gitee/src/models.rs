use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use skill_library_provider::{
    ChangedFile, FileEntry, FileKind, IssueComment, Member, PullRequest, PullRequestSummary,
    Release, RepositoryEvent, Tag, Workspace,
};

use crate::permissions::{permission_from_name, permission_from_repo, split_repo_path};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RepoResponse {
    #[serde(default)]
    pub(crate) full_name: Option<String>,
    #[serde(default)]
    pub(crate) path: Option<String>,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) default_branch: Option<String>,
    #[serde(default)]
    pub(crate) private: bool,
    #[serde(default)]
    pub(crate) html_url: Option<String>,
    #[serde(default)]
    pub(crate) permissions: Option<RepoPermissions>,
}

impl RepoResponse {
    pub(crate) fn into_workspace(self, provider: &str) -> Workspace {
        let full_name = self.full_name.clone().unwrap_or_else(|| {
            self.path
                .clone()
                .unwrap_or_else(|| self.name.clone().unwrap_or_default())
        });
        let (owner, repo) = split_repo_path(
            &full_name,
            self.path
                .as_deref()
                .or(self.name.as_deref())
                .unwrap_or_default(),
        );
        let visibility = if self.private { "private" } else { "public" }.to_owned();
        Workspace {
            provider: provider.to_owned(),
            owner,
            repo,
            remote_id: None,
            full_name,
            default_branch: self.default_branch.unwrap_or_else(|| "master".to_owned()),
            visibility: visibility.clone(),
            permission: permission_from_repo(self.permissions.as_ref(), &visibility),
            html_url: self.html_url,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RepoPermissions {
    #[serde(default)]
    pub(crate) admin: bool,
    #[serde(default)]
    pub(crate) push: bool,
    #[serde(default)]
    pub(crate) pull: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TreeResponse {
    #[serde(default)]
    pub(crate) tree: Vec<TreeEntryResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TreeEntryResponse {
    pub(crate) sha: String,
    pub(crate) path: String,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) size: Option<u64>,
}

impl From<TreeEntryResponse> for FileEntry {
    fn from(value: TreeEntryResponse) -> Self {
        let kind = match value.kind.as_str() {
            "tree" => FileKind::Directory,
            "blob" => FileKind::File,
            "commit" => FileKind::Submodule,
            _ => FileKind::File,
        };
        Self {
            path: value.path,
            kind,
            sha: value.sha,
            size: value.size,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TagResponse {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) commit: Option<TagCommitResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TagCommitResponse {
    pub(crate) sha: String,
    #[serde(default)]
    pub(crate) created_at: Option<DateTime<Utc>>,
}

impl From<TagResponse> for Tag {
    fn from(value: TagResponse) -> Self {
        let (sha, created_at) = value
            .commit
            .map(|commit| (commit.sha, commit.created_at))
            .unwrap_or_default();
        Self {
            name: value.name,
            sha,
            created_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReleaseResponse {
    #[serde(default)]
    pub(crate) id: Option<u64>,
    pub(crate) tag_name: String,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) body: Option<String>,
    #[serde(default)]
    pub(crate) prerelease: bool,
}

impl From<ReleaseResponse> for Release {
    fn from(value: ReleaseResponse) -> Self {
        Self {
            id: value
                .id
                .map(|id| id.to_string())
                .unwrap_or_else(|| value.tag_name.clone()),
            tag_name: value.tag_name,
            name: value.name,
            prerelease: value.prerelease,
            body: value.body,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CompareResponse {
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) ahead_by: Option<u32>,
    #[serde(default)]
    pub(crate) behind_by: Option<u32>,
    #[serde(default)]
    pub(crate) commits: Vec<serde_json::Value>,
    #[serde(default)]
    pub(crate) files: Vec<CompareFileResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CompareFileResponse {
    pub(crate) filename: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) patch: Option<String>,
}

impl From<CompareFileResponse> for ChangedFile {
    fn from(value: CompareFileResponse) -> Self {
        Self {
            filename: value.filename,
            status: value.status,
            patch: value.patch,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PermissionResponse {
    pub(crate) permission: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PullRequestResponse {
    #[serde(default)]
    pub(crate) number: Option<u64>,
    #[serde(default)]
    pub(crate) id: Option<u64>,
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default, alias = "url")]
    pub(crate) html_url: String,
    #[serde(default)]
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) draft: bool,
    #[serde(default)]
    pub(crate) merged_at: Option<String>,
    #[serde(default)]
    pub(crate) user: Option<GiteeUserResponse>,
    #[serde(default)]
    pub(crate) head: Option<PullRequestRefResponse>,
    #[serde(default)]
    pub(crate) base: Option<PullRequestRefResponse>,
    #[serde(default)]
    pub(crate) created_at: String,
    #[serde(default)]
    pub(crate) updated_at: String,
    #[serde(default, alias = "description")]
    pub(crate) body: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GiteeUserResponse {
    #[serde(default)]
    pub(crate) login: Option<String>,
    #[serde(default)]
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PullRequestRefResponse {
    #[serde(default, rename = "ref")]
    pub(crate) ref_name: String,
    #[serde(default)]
    pub(crate) repo: Option<PullRequestRefRepoResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PullRequestRefRepoResponse {
    #[serde(default)]
    pub(crate) full_name: Option<String>,
    #[serde(default)]
    pub(crate) path: Option<String>,
}

impl From<PullRequestResponse> for PullRequestSummary {
    fn from(value: PullRequestResponse) -> Self {
        let merged = value.state == "merged" || value.merged_at.is_some();
        let state = match value.state.as_str() {
            "open" | "opened" => "open",
            _ => "closed",
        };
        Self {
            number: value.number.or(value.id).unwrap_or(0),
            title: value.title,
            html_url: value.html_url,
            state: state.to_owned(),
            draft: value.draft,
            merged,
            author: value.user.and_then(|user| user.login.or(user.name)),
            head_ref: value
                .head
                .as_ref()
                .map(|head| head.ref_name.clone())
                .unwrap_or_default(),
            base_ref: value
                .base
                .as_ref()
                .map(|base| base.ref_name.clone())
                .unwrap_or_default(),
            head_repo: value
                .head
                .and_then(|head| head.repo.and_then(|repo| repo.full_name.or(repo.path))),
            base_repo: value
                .base
                .and_then(|base| base.repo.and_then(|repo| repo.full_name.or(repo.path))),
            created_at: value.created_at,
            updated_at: value.updated_at,
            body: value.body,
        }
    }
}

impl From<PullRequestResponse> for PullRequest {
    fn from(value: PullRequestResponse) -> Self {
        let state = match value.state.as_str() {
            "open" | "opened" => "open",
            _ => "closed",
        };
        Self {
            number: value.number.or(value.id).unwrap_or(0),
            title: value.title,
            html_url: value.html_url,
            state: state.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PullRequestFileResponse {
    #[serde(default, alias = "new_path")]
    pub(crate) filename: String,
    #[serde(default)]
    pub(crate) status: String,
    #[serde(default, alias = "diff")]
    pub(crate) patch: Option<String>,
}

impl From<PullRequestFileResponse> for ChangedFile {
    fn from(value: PullRequestFileResponse) -> Self {
        Self {
            filename: value.filename,
            status: value.status,
            patch: value.patch,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RepositoryEventResponse {
    #[serde(default)]
    pub(crate) id: Option<serde_json::Value>,
    #[serde(default, rename = "type")]
    pub(crate) event_type: Option<String>,
    #[serde(default)]
    pub(crate) actor: Option<GiteeUserResponse>,
    #[serde(default)]
    pub(crate) created_at: String,
    #[serde(default)]
    pub(crate) payload: Option<serde_json::Value>,
}

impl From<RepositoryEventResponse> for RepositoryEvent {
    fn from(value: RepositoryEventResponse) -> Self {
        let event_type = value.event_type.unwrap_or_else(|| "unknown".to_owned());
        let id = value
            .id
            .as_ref()
            .and_then(|id| {
                id.as_str()
                    .map(str::to_owned)
                    .or_else(|| id.as_u64().map(|value| value.to_string()))
            })
            .unwrap_or_else(|| value.created_at.clone());
        let (summary, html_url) = match (event_type.as_str(), value.payload.as_ref()) {
            ("PushEvent", Some(payload)) => {
                let ref_name = payload.get("ref").and_then(|v| v.as_str()).unwrap_or("");
                let count = payload
                    .get("commits")
                    .and_then(|v| v.as_array())
                    .map(|commits| commits.len())
                    .unwrap_or(0);
                (format!("Pushed {count} commit(s) to {ref_name}"), None)
            }
            ("PullRequestEvent", Some(payload)) => {
                let action = payload.get("action").and_then(|v| v.as_str()).unwrap_or("");
                let pr = payload.get("pull_request");
                let title = pr
                    .and_then(|p| p.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let url = pr
                    .and_then(|p| p.get("html_url").or_else(|| p.get("url")))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                (format!("Pull request {action}: {title}"), url)
            }
            (other, _) => (other.to_owned(), None),
        };
        Self {
            id,
            event_type,
            actor: value.actor.and_then(|actor| actor.login.or(actor.name)),
            created_at: value.created_at,
            summary,
            html_url,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CollaboratorResponse {
    #[serde(default)]
    pub(crate) login: Option<String>,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) avatar_url: Option<String>,
    #[serde(default)]
    pub(crate) permission: Option<String>,
    #[serde(default)]
    pub(crate) permissions: Option<RepoPermissions>,
}

impl From<CollaboratorResponse> for Member {
    fn from(value: CollaboratorResponse) -> Self {
        let role = value
            .permission
            .as_deref()
            .map(permission_from_name)
            .unwrap_or_else(|| permission_from_repo(value.permissions.as_ref(), "private"));
        Self {
            login: value.login.or(value.name).unwrap_or_default(),
            role,
            avatar_url: value.avatar_url,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClosePullRequestRequest {
    pub(crate) state: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PullRequestCommentRequest<'a> {
    pub(crate) body: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PullRequestCommentResponse {
    pub(crate) id: u64,
    #[serde(default, alias = "url")]
    pub(crate) html_url: String,
    #[serde(default)]
    pub(crate) body: Option<String>,
    #[serde(default)]
    pub(crate) created_at: String,
}

impl From<PullRequestCommentResponse> for IssueComment {
    fn from(value: PullRequestCommentResponse) -> Self {
        Self {
            id: value.id,
            html_url: value.html_url,
            body: value.body,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CollaboratorRequest<'a> {
    pub(crate) permission: &'a str,
}
