use chrono::{DateTime, Utc};
use serde::Deserialize;
use skill_library_provider::{ChangedFile, FileEntry, FileKind, Release, Tag, Workspace};

use crate::permissions::{permission_from_project, split_project_path};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProjectResponse {
    pub(crate) path: String,
    pub(crate) path_with_namespace: String,
    #[serde(default)]
    pub(crate) default_branch: Option<String>,
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
            full_name: self.path_with_namespace,
            default_branch: self.default_branch.unwrap_or_else(|| "HEAD".to_owned()),
            visibility: self.visibility.clone(),
            permission: permission_from_project(self.permissions.as_ref(), &self.visibility),
            html_url: self.web_url,
        }
    }
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
}
