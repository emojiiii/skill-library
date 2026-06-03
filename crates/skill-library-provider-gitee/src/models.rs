use chrono::{DateTime, Utc};
use serde::Deserialize;
use skill_library_provider::{ChangedFile, FileEntry, FileKind, Release, Tag, Workspace};

use crate::permissions::{permission_from_repo, split_repo_path};

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
