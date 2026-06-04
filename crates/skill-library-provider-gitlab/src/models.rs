use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use skill_library_provider::{
    ChangedFile, FileEntry, FileKind, IssueComment, Member, PullRequestSummary, Release,
    RepositoryEvent, Tag, Workspace,
};

use crate::permissions::{
    permission_from_access_level, permission_from_project, split_project_path,
};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectResponse {
    pub(crate) id: u64,
    pub(crate) path: String,
    pub(crate) path_with_namespace: String,
    #[serde(default)]
    pub(crate) default_branch: Option<String>,
    #[serde(default = "default_visibility")]
    pub(crate) visibility: String,
    #[serde(default)]
    pub(crate) web_url: Option<String>,
    #[serde(default)]
    pub(crate) permissions: Option<ProjectPermissions>,
}

impl ProjectResponse {
    pub(crate) fn into_workspace(self, provider: &str) -> Workspace {
        let (owner, repo) = split_project_path(&self.path_with_namespace, &self.path);
        Workspace {
            provider: provider.to_owned(),
            owner,
            repo,
            remote_id: Some(self.id.to_string()),
            full_name: self.path_with_namespace,
            default_branch: self.default_branch.unwrap_or_else(|| "HEAD".to_owned()),
            visibility: self.visibility.clone(),
            permission: permission_from_project(self.permissions.as_ref(), &self.visibility),
            html_url: self.web_url,
        }
    }
}

fn default_visibility() -> String {
    "private".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectPermissions {
    #[serde(default)]
    pub(crate) project_access: Option<ProjectAccess>,
    #[serde(default)]
    pub(crate) group_access: Option<ProjectAccess>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectAccess {
    pub(crate) access_level: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TreeEntryResponse {
    pub(crate) id: String,
    pub(crate) path: String,
    #[serde(rename = "type")]
    pub(crate) kind: String,
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
            sha: value.id,
            size: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TagResponse {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) target: Option<String>,
    #[serde(default)]
    pub(crate) commit: Option<TagCommitResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TagCommitResponse {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) created_at: Option<DateTime<Utc>>,
}

impl From<TagResponse> for Tag {
    fn from(value: TagResponse) -> Self {
        let (sha, created_at) = value
            .commit
            .map(|commit| (commit.id, commit.created_at))
            .unwrap_or_else(|| (value.target.unwrap_or_default(), None));
        Self {
            name: value.name,
            sha,
            created_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReleaseResponse {
    pub(crate) tag_name: String,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) upcoming_release: bool,
}

impl From<ReleaseResponse> for Release {
    fn from(value: ReleaseResponse) -> Self {
        Self {
            id: value.tag_name.clone(),
            tag_name: value.tag_name,
            name: value.name,
            prerelease: value.upcoming_release,
            body: value.description,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CompareResponse {
    #[serde(default)]
    pub(crate) commits: Vec<serde_json::Value>,
    #[serde(default)]
    pub(crate) diffs: Vec<CompareDiffResponse>,
    #[serde(default)]
    pub(crate) compare_same_ref: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CompareDiffResponse {
    pub(crate) old_path: String,
    pub(crate) new_path: String,
    #[serde(default)]
    pub(crate) diff: Option<String>,
    #[serde(default)]
    pub(crate) new_file: bool,
    #[serde(default)]
    pub(crate) deleted_file: bool,
    #[serde(default)]
    pub(crate) renamed_file: bool,
}

impl From<CompareDiffResponse> for ChangedFile {
    fn from(value: CompareDiffResponse) -> Self {
        let status = if value.new_file {
            "added"
        } else if value.deleted_file {
            "removed"
        } else if value.renamed_file {
            "renamed"
        } else {
            "modified"
        };
        Self {
            filename: if value.new_path.is_empty() {
                value.old_path
            } else {
                value.new_path
            },
            status: status.to_owned(),
            patch: value.diff,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MemberResponse {
    pub(crate) username: String,
    pub(crate) access_level: u32,
    #[serde(default)]
    pub(crate) avatar_url: Option<String>,
}

impl From<MemberResponse> for Member {
    fn from(value: MemberResponse) -> Self {
        Self {
            login: value.username,
            role: permission_from_access_level(value.access_level),
            avatar_url: value.avatar_url,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MergeRequestResponse {
    pub(crate) iid: u64,
    pub(crate) title: String,
    pub(crate) web_url: String,
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) draft: bool,
    #[serde(default)]
    pub(crate) work_in_progress: bool,
    #[serde(default)]
    pub(crate) merged_at: Option<String>,
    #[serde(default)]
    pub(crate) author: Option<GitLabUserResponse>,
    #[serde(default)]
    pub(crate) source_branch: String,
    #[serde(default)]
    pub(crate) target_branch: String,
    #[serde(default)]
    pub(crate) source_project_id: Option<u64>,
    #[serde(default)]
    pub(crate) target_project_id: Option<u64>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitLabUserResponse {
    #[serde(default)]
    pub(crate) id: Option<u64>,
    pub(crate) username: String,
}

impl From<MergeRequestResponse> for PullRequestSummary {
    fn from(value: MergeRequestResponse) -> Self {
        let merged = value.state == "merged" || value.merged_at.is_some();
        Self {
            number: value.iid,
            title: value.title,
            html_url: value.web_url,
            state: if value.state == "opened" {
                "open".to_owned()
            } else {
                "closed".to_owned()
            },
            draft: value.draft || value.work_in_progress,
            merged,
            author: value.author.map(|author| author.username),
            head_ref: value.source_branch,
            base_ref: value.target_branch,
            head_repo: value.source_project_id.map(|id| id.to_string()),
            base_repo: value.target_project_id.map(|id| id.to_string()),
            created_at: value.created_at,
            updated_at: value.updated_at,
            body: value.description,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MergeRequestChangesResponse {
    #[serde(default)]
    pub(crate) changes: Vec<CompareDiffResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MergeRequestCloseRequest {
    pub(crate) state_event: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MergeRequestNoteRequest<'a> {
    pub(crate) body: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MergeRequestNoteResponse {
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) body: Option<String>,
    pub(crate) created_at: String,
}

impl From<MergeRequestNoteResponse> for IssueComment {
    fn from(value: MergeRequestNoteResponse) -> Self {
        Self {
            id: value.id,
            html_url: String::new(),
            body: value.body,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MemberAccessRequest {
    pub(crate) user_id: u64,
    pub(crate) access_level: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MemberUpdateRequest {
    pub(crate) access_level: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProjectInvitationRequest<'a> {
    pub(crate) email: &'a str,
    pub(crate) access_level: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectEventResponse {
    #[serde(default)]
    pub(crate) id: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) action_name: Option<String>,
    #[serde(default)]
    pub(crate) target_type: Option<String>,
    #[serde(default)]
    pub(crate) target_title: Option<String>,
    #[serde(default)]
    pub(crate) target_url: Option<String>,
    #[serde(default)]
    pub(crate) author: Option<GitLabUserResponse>,
    pub(crate) created_at: String,
    #[serde(default)]
    pub(crate) push_data: Option<ProjectPushDataResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectPushDataResponse {
    #[serde(default)]
    pub(crate) commit_count: Option<u64>,
    #[serde(default)]
    pub(crate) ref_name: Option<String>,
    #[serde(default)]
    pub(crate) action: Option<String>,
}

impl From<ProjectEventResponse> for RepositoryEvent {
    fn from(value: ProjectEventResponse) -> Self {
        let id = value
            .id
            .as_ref()
            .and_then(|id| {
                id.as_str()
                    .map(str::to_owned)
                    .or_else(|| id.as_u64().map(|value| value.to_string()))
            })
            .unwrap_or_else(|| value.created_at.clone());
        let actor = value.author.map(|author| author.username);
        let (event_type, summary) = if let Some(push) = value.push_data {
            let count = push.commit_count.unwrap_or(0);
            let ref_name = push.ref_name.unwrap_or_default();
            let action = push.action.unwrap_or_else(|| "pushed".to_owned());
            (
                "PushEvent".to_owned(),
                format!("{action} {count} commit(s) to {ref_name}"),
            )
        } else {
            let target_type = value.target_type.unwrap_or_default();
            let action = value.action_name.unwrap_or_else(|| "updated".to_owned());
            let title = value.target_title.unwrap_or_default();
            let event_type = match target_type.as_str() {
                "MergeRequest" => "PullRequestEvent",
                "Release" => "ReleaseEvent",
                _ => "CreateEvent",
            };
            (
                event_type.to_owned(),
                format!("{action} {title}").trim().to_owned(),
            )
        };
        Self {
            id,
            event_type,
            actor,
            created_at: value.created_at,
            summary,
            html_url: value.target_url,
        }
    }
}
