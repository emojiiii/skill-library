use async_trait::async_trait;
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skill_library_core::{ProviderInstance, ProviderKind, WorkspaceRef};
use skill_library_provider::{
    ChangedFile, FileBlob, FileEntry, FileKind, GitRef, Invitation, InvitationInput, Member, Page,
    PageOpts, PermissionLevel, Provider, ProviderCapabilities, ProviderError, PullRequest,
    PullRequestInput, RefComparison, Release, Result, Tag, WebhookConfig, WebhookHandle, Workspace,
};
use std::path::Path;
use tracing;

mod http;
mod models;
mod traits;
mod util;

#[cfg(test)]
mod tests;

pub mod scan;

use http::{map_empty_response, map_response, parse_scope_header, provider_error_from_status};
use models::{
    ClosePullRequestRequest, CollaboratorInvitationRequest, CollaboratorInvitationResponse,
    CollaboratorResponse, CommitListItemResponse, CompareResponse, ContentResponse,
    CreateGitRefRequest, CreatePullRequestRequest, DeviceCodeRequest, DeviceTokenRequest,
    GitReferenceResponse, IssueCommentRequest, MergePullRequestRequest, MergePullRequestResponse,
    PermissionResponse, PullRequestFileResponse, PullRequestListItemResponse, PullRequestResponse,
    PutContentRequest, PutContentResponse, ReleaseResponse, RepoResponse, RepositoryEventResponse,
    RepositoryInvitationResponse, TagResponse, TreeResponse, WebhookResponse,
};
pub use models::{
    CommitSummary, DeviceCodeResponse, DeviceTokenResponse, GitHubArchiveDownload,
    GitHubPublishFile, GitHubPublishInput, GitHubPublishResult, GitHubTokenInfo,
    GitHubUploadedFile, GitHubUser, IssueComment, PullRequestQueryState, PullRequestSummary,
    RepositoryEvent, RepositoryInvitation,
};
use util::{
    extract_tarball, github_permission_role, github_webhook_request, urlencoding_simple,
    validate_branch_ref, validate_repo_path,
};

fn log_snippet(value: &str) -> String {
    value.chars().take(200).collect()
}

pub struct GitHubProvider {
    client: reqwest::Client,
    api_base: String,
    instance_id: String,
    authenticated: bool,
}

impl GitHubProvider {
    pub fn new(token: impl Into<String>) -> Result<Self> {
        Self::with_base_url("https://api.github.com", Some(token.into()))
    }

    pub fn anonymous(api_base: impl Into<String>) -> Result<Self> {
        Self::with_base_url(api_base, None)
    }

    pub fn for_instance(instance: &ProviderInstance, token: Option<String>) -> Result<Self> {
        if !matches!(instance.kind, ProviderKind::GitHub) {
            return Err(ProviderError::InvalidResponse(format!(
                "provider instance {} is not a GitHub provider",
                instance.id
            )));
        }
        Self::with_instance_base_url(instance.id.clone(), instance.api_base_url.clone(), token)
    }

    pub fn with_base_url(api_base: impl Into<String>, token: Option<String>) -> Result<Self> {
        Self::with_instance_base_url("github.com", api_base, token)
    }

    pub fn with_instance_base_url(
        instance_id: impl Into<String>,
        api_base: impl Into<String>,
        token: Option<String>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("skill-library/0.1"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        let authenticated = token.as_ref().is_some_and(|token| !token.trim().is_empty());
        if let Some(token) = token.filter(|token| !token.trim().is_empty()) {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?,
            );
        }
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;

        Ok(Self {
            client,
            api_base: api_base.into().trim_end_matches('/').to_owned(),
            instance_id: instance_id.into(),
            authenticated,
        })
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-github", method = "GET", path);
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        map_response(path, "GET", response).await
    }

    async fn post_json<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-github", method = "POST", path);
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        map_response(path, "POST", response).await
    }

    async fn put_json<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-github", method = "PUT", path);
        let response = self
            .client
            .put(url)
            .json(body)
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        map_response(path, "PUT", response).await
    }

    async fn patch_json<B: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-github", method = "PATCH", path);
        let response = self
            .client
            .patch(url)
            .json(body)
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        map_response(path, "PATCH", response).await
    }

    async fn delete_empty(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.api_base, path);
        tracing::debug!(target: "skill-library-github", method = "DELETE", path);
        let response =
            self.client
                .delete(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        map_empty_response(path, "DELETE", response).await
    }

    pub async fn current_user(&self) -> Result<GitHubUser> {
        self.get_json("/user").await
    }

    /// POST a GraphQL query to https://api.github.com/graphql with the same auth.
    /// Used to batch-fetch many manifest blobs in a single round-trip during scan.
    pub async fn graphql<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        // GitHub's GraphQL endpoint lives at api.github.com/graphql, regardless
        // of the REST api_base, but during tests we point both at the mock server
        // so we honour api_base here.
        let url = format!("{}/graphql", self.api_base);
        tracing::debug!(target: "skill-library-github", method = "POST", path = "/graphql");
        let response = self
            .client
            .post(url)
            .json(&serde_json::json!({ "query": query, "variables": variables }))
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        if !status.is_success() {
            let snippet = log_snippet(&String::from_utf8_lossy(&bytes));
            tracing::warn!(
                target: "skill-library-github",
                method = "POST",
                path = "/graphql",
                status = status.as_u16(),
                body = %snippet,
                "non-success response"
            );
            return Err(provider_error_from_status(
                status,
                format!("POST /graphql ({status}): {snippet}"),
            ));
        }
        // GraphQL itself returns 200 even on errors; surface them.
        #[derive(Deserialize)]
        struct Envelope<T> {
            data: Option<T>,
            #[serde(default)]
            errors: Vec<serde_json::Value>,
        }
        let envelope: Envelope<T> = serde_json::from_slice(&bytes).map_err(|err| {
            let snippet = log_snippet(&String::from_utf8_lossy(&bytes));
            tracing::error!(
                target: "skill-library-github",
                method = "POST",
                path = "/graphql",
                status = status.as_u16(),
                body = %snippet,
                error = %err,
                "deserialize failed"
            );
            ProviderError::InvalidResponse(format!(
                "POST /graphql: deserialize failed: {err} — body: {snippet}"
            ))
        })?;
        if !envelope.errors.is_empty() {
            let summary = envelope
                .errors
                .iter()
                .filter_map(|err| err.get("message").and_then(|m| m.as_str()))
                .collect::<Vec<_>>()
                .join("; ");
            tracing::warn!(target: "skill-library-github", errors = %summary, "graphql errors");
            return Err(ProviderError::InvalidResponse(format!(
                "GraphQL errors: {summary}"
            )));
        }
        envelope
            .data
            .ok_or_else(|| ProviderError::InvalidResponse("graphql: missing data".to_owned()))
    }

    /// Fetch text contents of multiple files at once via GraphQL. The returned
    /// vec is in the same order as `paths`, with `None` for files that don't
    /// exist or aren't text.
    pub async fn batch_fetch_text_files(
        &self,
        reference: &WorkspaceRef,
        ref_name: &str,
        paths: &[String],
    ) -> Result<Vec<Option<String>>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        // Build a query that asks for one alias per path:
        //   f0: object(expression: "main:foo/manifest.yaml") { ... on Blob { text } }
        //   f1: object(expression: "main:bar/manifest.yaml") { ... on Blob { text } }
        let mut field_lines = String::new();
        for (i, path) in paths.iter().enumerate() {
            // GraphQL string escape — GitHub paths shouldn't contain quotes/backslashes
            // but escape defensively.
            let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
            let escaped_ref = ref_name.replace('\\', "\\\\").replace('"', "\\\"");
            field_lines.push_str(&format!(
                "  f{i}: object(expression: \"{escaped_ref}:{escaped}\") {{ ... on Blob {{ text isBinary byteSize }} }}\n"
            ));
        }
        let query = format!(
            "query($owner: String!, $repo: String!) {{\n  repository(owner: $owner, name: $repo) {{\n{field_lines}  }}\n}}\n"
        );
        let variables = serde_json::json!({
            "owner": reference.owner,
            "repo": reference.repo,
        });

        let data: serde_json::Value = self.graphql(&query, variables).await?;
        let repo = data.get("repository").ok_or_else(|| {
            ProviderError::InvalidResponse("graphql: missing repository field".to_owned())
        })?;

        let mut out = Vec::with_capacity(paths.len());
        for i in 0..paths.len() {
            let alias = format!("f{i}");
            let entry = repo.get(&alias);
            let text = entry.and_then(|node| {
                if node.is_null() {
                    return None;
                }
                let is_binary = node
                    .get("isBinary")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if is_binary {
                    return None;
                }
                node.get("text").and_then(|v| v.as_str()).map(str::to_owned)
            });
            out.push(text);
        }
        Ok(out)
    }

    pub async fn validate_token(&self) -> Result<GitHubTokenInfo> {
        let url = format!("{}/user", self.api_base);
        tracing::debug!(target: "skill-library-github", method = "GET", path = "/user");
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        let status = response.status();
        let scopes = response
            .headers()
            .get("x-oauth-scopes")
            .and_then(|value| value.to_str().ok())
            .map(parse_scope_header)
            .unwrap_or_default();
        let bytes = response
            .bytes()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        if !status.is_success() {
            let body = log_snippet(&String::from_utf8_lossy(&bytes));
            tracing::warn!(
                target: "skill-library-github",
                method = "GET",
                path = "/user",
                status = status.as_u16(),
                body = %body,
                "non-success response"
            );
            return Err(provider_error_from_status(
                status,
                format!("GET /user ({status}): {body}"),
            ));
        }

        let user = serde_json::from_slice::<GitHubUser>(&bytes).map_err(|err| {
            let body = log_snippet(&String::from_utf8_lossy(&bytes));
            tracing::error!(
                target: "skill-library-github",
                method = "GET",
                path = "/user",
                status = status.as_u16(),
                body = %body,
                error = %err,
                "deserialize failed"
            );
            ProviderError::InvalidResponse(format!(
                "GET /user ({status}): deserialize failed: {err} - body: {body}"
            ))
        })?;
        Ok(GitHubTokenInfo { user, scopes })
    }

    pub async fn download_tarball(
        &self,
        reference: &WorkspaceRef,
        ref_name: &str,
        destination: impl AsRef<Path>,
    ) -> Result<GitHubArchiveDownload> {
        self.download_tarball_with_progress(reference, ref_name, destination, |_, _| {})
            .await
    }

    /// Stream the repository tarball, invoking `on_progress(downloaded, total)`
    /// as bytes arrive so callers can surface real download progress.
    ///
    /// `total` is the response `Content-Length` when the server provides it.
    /// GitHub's codeload redirect target frequently uses chunked transfer with
    /// no length, in which case `total` is `None` and only the running byte
    /// count is meaningful (the UI should fall back to an indeterminate bar).
    pub async fn download_tarball_with_progress<F>(
        &self,
        reference: &WorkspaceRef,
        ref_name: &str,
        destination: impl AsRef<Path>,
        mut on_progress: F,
    ) -> Result<GitHubArchiveDownload>
    where
        F: FnMut(u64, Option<u64>),
    {
        use futures::StreamExt;

        let url = if !self.authenticated && self.api_base == "https://api.github.com" {
            format!(
                "https://codeload.github.com/{}/{}/tar.gz/{}",
                reference.owner, reference.repo, ref_name
            )
        } else {
            format!(
                "{}/repos/{}/{}/tarball/{}",
                self.api_base, reference.owner, reference.repo, ref_name
            )
        };
        let log_path = format!(
            "/repos/{}/{}/tarball/{}",
            reference.owner, reference.repo, ref_name
        );
        tracing::debug!(target: "skill-library-github", method = "GET", path = %log_path);
        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| status.to_string());
            let body = log_snippet(&body);
            tracing::warn!(
                target: "skill-library-github",
                method = "GET",
                path = %log_path,
                status = status.as_u16(),
                body = %body,
                "non-success response"
            );
            return Err(provider_error_from_status(
                status,
                format!("GET {log_path} ({status}): {body}"),
            ));
        }

        let total = response.content_length();
        let mut buf: Vec<u8> = Vec::with_capacity(total.unwrap_or(0) as usize);
        let mut downloaded: u64 = 0;
        on_progress(0, total);

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
            downloaded += chunk.len() as u64;
            buf.extend_from_slice(&chunk);
            on_progress(downloaded, total);
        }

        let sha256 = hex::encode(Sha256::digest(&buf));
        let extracted_root = extract_tarball(&buf, destination.as_ref())?;
        Ok(GitHubArchiveDownload {
            ref_name: ref_name.to_owned(),
            sha256,
            bytes: buf.len() as u64,
            extracted_root,
        })
    }

    pub async fn publish_files_pull_request(
        &self,
        reference: &WorkspaceRef,
        input: GitHubPublishInput,
    ) -> Result<GitHubPublishResult> {
        let workspace = self.get_workspace(reference).await?;
        let base = input.base.unwrap_or(workspace.default_branch);
        let base_ref: GitReferenceResponse = self
            .get_json(&format!(
                "/repos/{}/{}/git/ref/heads/{}",
                reference.owner, reference.repo, base
            ))
            .await?;
        let branch_ref = format!("refs/heads/{}", input.branch_name);
        let _created: GitReferenceResponse = self
            .post_json(
                &format!("/repos/{}/{}/git/refs", reference.owner, reference.repo),
                &CreateGitRefRequest {
                    reference: branch_ref,
                    sha: base_ref.object.sha.clone(),
                },
            )
            .await?;

        let mut uploaded = Vec::new();
        for file in input.files {
            validate_repo_path(&file.path)?;
            let existing = match <Self as Provider>::read_file(
                self,
                reference,
                &GitRef::Branch(input.branch_name.clone()),
                &file.path,
            )
            .await
            {
                Ok(blob) => Some(blob),
                Err(ProviderError::NotFound { .. }) => None,
                Err(err) => return Err(err),
            };
            if existing
                .as_ref()
                .map(|blob| blob.bytes == file.bytes)
                .unwrap_or(false)
            {
                continue;
            }
            let content = base64::engine::general_purpose::STANDARD.encode(&file.bytes);
            let response: PutContentResponse = self
                .put_json(
                    &format!(
                        "/repos/{}/{}/contents/{}",
                        reference.owner, reference.repo, file.path
                    ),
                    &PutContentRequest {
                        message: input.commit_message.clone(),
                        content,
                        branch: input.branch_name.clone(),
                        sha: existing.map(|blob| blob.sha),
                    },
                )
                .await?;
            uploaded.push(GitHubUploadedFile {
                path: file.path,
                sha: response.content.sha,
            });
        }

        let pr: PullRequestResponse = self
            .post_json(
                &format!("/repos/{}/{}/pulls", reference.owner, reference.repo),
                &CreatePullRequestRequest {
                    title: input.title,
                    head: input.branch_name,
                    base,
                    body: input.body,
                    draft: false,
                },
            )
            .await?;

        Ok(GitHubPublishResult {
            pull_request: pr.into(),
            uploaded,
        })
    }

    /// Create or update a file directly on an existing branch with a single
    /// commit. This is intentionally narrower than `publish_files_pull_request`
    /// for small shared metadata files such as `.reviews/{skill}.json`.
    pub async fn put_file_content(
        &self,
        reference: &WorkspaceRef,
        branch: &str,
        path: &str,
        bytes: &[u8],
        message: &str,
    ) -> Result<GitHubUploadedFile> {
        validate_repo_path(path)?;
        let git_ref = GitRef::Branch(branch.to_owned());
        let existing_sha =
            match <Self as Provider>::read_file(self, reference, &git_ref, path).await {
                Ok(blob) => Some(blob.sha),
                Err(ProviderError::NotFound { .. }) => None,
                Err(err) => return Err(err),
            };
        let content = base64::engine::general_purpose::STANDARD.encode(bytes);
        let response: PutContentResponse = self
            .put_json(
                &format!(
                    "/repos/{}/{}/contents/{}",
                    reference.owner, reference.repo, path
                ),
                &PutContentRequest {
                    message: message.to_owned(),
                    content,
                    branch: branch.to_owned(),
                    sha: existing_sha,
                },
            )
            .await?;

        Ok(GitHubUploadedFile {
            path: path.to_owned(),
            sha: response.content.sha,
        })
    }

    pub async fn invite_collaborator(
        &self,
        reference: &WorkspaceRef,
        input: InvitationInput,
    ) -> Result<Invitation> {
        let role = github_permission_role(&input.role)?;
        let invitation: CollaboratorInvitationResponse = self
            .put_json(
                &format!(
                    "/repos/{}/{}/collaborators/{}",
                    reference.owner, reference.repo, input.login_or_email
                ),
                &CollaboratorInvitationRequest { permission: role },
            )
            .await?;
        Ok(Invitation {
            id: invitation.id.to_string(),
            login_or_email: invitation
                .invitee
                .map(|invitee| invitee.login)
                .unwrap_or(input.login_or_email),
            state: invitation.state.unwrap_or_else(|| "pending".to_owned()),
        })
    }

    pub async fn list_pull_requests(
        &self,
        reference: &WorkspaceRef,
        state: PullRequestQueryState,
    ) -> Result<Vec<PullRequestSummary>> {
        let state_param = match state {
            PullRequestQueryState::Open => "open",
            PullRequestQueryState::Closed => "closed",
            PullRequestQueryState::All => "all",
        };
        let path = format!(
            "/repos/{}/{}/pulls?state={state_param}&per_page=50&sort=updated&direction=desc",
            reference.owner, reference.repo
        );
        let raw: Vec<PullRequestListItemResponse> = self.get_json(&path).await?;
        Ok(raw.into_iter().map(PullRequestSummary::from).collect())
    }

    pub async fn list_pull_request_files(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<Vec<ChangedFile>> {
        let raw: Vec<PullRequestFileResponse> = self
            .get_json(&format!(
                "/repos/{}/{}/pulls/{number}/files?per_page=100",
                reference.owner, reference.repo
            ))
            .await?;
        Ok(raw
            .into_iter()
            .map(|file| ChangedFile {
                filename: file.filename,
                status: file.status,
                patch: file.patch,
            })
            .collect())
    }

    pub async fn close_pull_request(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<PullRequestSummary> {
        let pr: PullRequestListItemResponse = self
            .patch_json(
                &format!(
                    "/repos/{}/{}/pulls/{number}",
                    reference.owner, reference.repo
                ),
                &ClosePullRequestRequest { state: "closed" },
            )
            .await?;
        Ok(pr.into())
    }

    pub async fn add_pull_request_comment(
        &self,
        reference: &WorkspaceRef,
        number: u64,
        body: &str,
    ) -> Result<IssueComment> {
        self.post_json(
            &format!(
                "/repos/{}/{}/issues/{number}/comments",
                reference.owner, reference.repo
            ),
            &IssueCommentRequest { body },
        )
        .await
    }

    pub async fn delete_branch(&self, reference: &WorkspaceRef, branch: &str) -> Result<()> {
        validate_branch_ref(branch)?;
        self.delete_empty(&format!(
            "/repos/{}/{}/git/refs/heads/{}",
            reference.owner,
            reference.repo,
            urlencoding_simple(branch)
        ))
        .await
    }

    /// List commits that touched a specific path (per-skill timeline).
    /// Uses GitHub /repos/{owner}/{repo}/commits?path=... — server-side filter,
    /// so we never download the whole repo history.
    pub async fn list_path_commits(
        &self,
        reference: &WorkspaceRef,
        path: &str,
        ref_name: Option<&str>,
        limit: u32,
    ) -> Result<Vec<CommitSummary>> {
        let limit = limit.clamp(1, 100);
        let mut query = format!("path={}&per_page={limit}", urlencoding_simple(path));
        if let Some(sha) = ref_name {
            query.push_str(&format!("&sha={}", urlencoding_simple(sha)));
        }
        let api_path = format!(
            "/repos/{}/{}/commits?{query}",
            reference.owner, reference.repo
        );
        let raw: Vec<CommitListItemResponse> = self.get_json(&api_path).await?;
        Ok(raw.into_iter().map(CommitSummary::from).collect())
    }

    pub async fn list_branches(&self, reference: &WorkspaceRef) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct BranchResponse {
            name: String,
        }
        let path = format!(
            "/repos/{}/{}/branches?per_page=100",
            reference.owner, reference.repo
        );
        let branches: Vec<BranchResponse> = self.get_json(&path).await?;
        Ok(branches.into_iter().map(|b| b.name).collect())
    }

    pub async fn list_repository_events(
        &self,
        reference: &WorkspaceRef,
    ) -> Result<Vec<RepositoryEvent>> {
        let path = format!(
            "/repos/{}/{}/events?per_page=30",
            reference.owner, reference.repo
        );
        let raw: Vec<RepositoryEventResponse> = self.get_json(&path).await?;
        Ok(raw.into_iter().map(RepositoryEvent::from).collect())
    }

    pub async fn list_user_repository_invitations(&self) -> Result<Vec<RepositoryInvitation>> {
        let raw: Vec<RepositoryInvitationResponse> = self
            .get_json("/user/repository_invitations?per_page=50")
            .await?;
        Ok(raw.into_iter().map(RepositoryInvitation::from).collect())
    }

    pub async fn accept_user_repository_invitation(&self, invitation_id: u64) -> Result<()> {
        let url = format!(
            "{}/user/repository_invitations/{invitation_id}",
            self.api_base
        );
        let path = format!("/user/repository_invitations/{invitation_id}");
        tracing::debug!(target: "skill-library-github", method = "PATCH", path = %path);
        let response =
            self.client
                .patch(url)
                .send()
                .await
                .map_err(|err| ProviderError::NetworkError {
                    cause: err.to_string(),
                })?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_else(|_| status.to_string());
        let body = log_snippet(&body);
        tracing::warn!(
            target: "skill-library-github",
            method = "PATCH",
            path = %path,
            status = status.as_u16(),
            body = %body,
            "non-success response"
        );
        Err(provider_error_from_status(
            status,
            format!("PATCH {path} ({status}): {body}"),
        ))
    }

    pub async fn create_webhook(
        &self,
        reference: &WorkspaceRef,
        config: WebhookConfig,
    ) -> Result<WebhookHandle> {
        let request = github_webhook_request(config)?;
        let hook: WebhookResponse = self
            .post_json(
                &format!("/repos/{}/{}/hooks", reference.owner, reference.repo),
                &request,
            )
            .await?;
        Ok(WebhookHandle {
            id: hook.id.to_string(),
            url: hook.config.and_then(|config| config.url),
        })
    }

    pub async fn start_device_flow(client_id: &str, scopes: &[&str]) -> Result<DeviceCodeResponse> {
        let client = reqwest::Client::builder()
            .user_agent("skill-library/0.1")
            .build()
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        let response = client
            .post("https://github.com/login/device/code")
            .header(ACCEPT, "application/json")
            .form(&DeviceCodeRequest {
                client_id,
                scope: &scopes.join(" "),
            })
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        map_response("/login/device/code", "POST", response).await
    }

    pub async fn poll_device_flow(
        client_id: &str,
        device_code: &str,
    ) -> Result<DeviceTokenResponse> {
        let client = reqwest::Client::builder()
            .user_agent("skill-library/0.1")
            .build()
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        let response = client
            .post("https://github.com/login/oauth/access_token")
            .header(ACCEPT, "application/json")
            .form(&DeviceTokenRequest {
                client_id,
                device_code,
                grant_type: "urn:ietf:params:oauth:grant-type:device_code",
            })
            .send()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        map_response("/login/oauth/access_token", "POST", response).await
    }
}

#[async_trait]
impl Provider for GitHubProvider {
    fn id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::github()
    }

    async fn list_workspaces(&self, opts: PageOpts) -> Result<Page<Workspace>> {
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let repos: Vec<RepoResponse> = self
            .get_json(&format!(
                "/user/repos?per_page={per_page}&page={page}&affiliation=owner,collaborator,organization_member&sort=updated"
            ))
            .await?;
        Ok(Page::single(
            repos.into_iter().map(Workspace::from).collect(),
        ))
    }

    async fn get_workspace(&self, reference: &WorkspaceRef) -> Result<Workspace> {
        let repo: RepoResponse = self
            .get_json(&format!("/repos/{}/{}", reference.owner, reference.repo))
            .await?;
        Ok(repo.into())
    }

    async fn list_files(&self, reference: &WorkspaceRef, at: &GitRef) -> Result<Vec<FileEntry>> {
        let tree: TreeResponse = self
            .get_json(&format!(
                "/repos/{}/{}/git/trees/{}?recursive=1",
                reference.owner,
                reference.repo,
                at.value()
            ))
            .await?;
        if tree.truncated {
            tracing::warn!(
                target: "skill-library-github",
                owner = %reference.owner,
                repo = %reference.repo,
                "git tree response was truncated; some skills may be missing — repo exceeds GitHub's tree size limit"
            );
        }
        Ok(tree
            .tree
            .into_iter()
            .map(|entry| FileEntry {
                path: entry.path,
                kind: match entry.kind.as_str() {
                    "tree" => FileKind::Directory,
                    "commit" => FileKind::Submodule,
                    _ => FileKind::File,
                },
                sha: entry.sha.unwrap_or_default(),
                size: entry.size,
            })
            .collect())
    }

    async fn read_file(
        &self,
        reference: &WorkspaceRef,
        at: &GitRef,
        path: &str,
    ) -> Result<FileBlob> {
        let blob: ContentResponse = self
            .get_json(&format!(
                "/repos/{}/{}/contents/{}?ref={}",
                reference.owner,
                reference.repo,
                path,
                at.value()
            ))
            .await?;
        let content = blob.content.replace('\n', "");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(content.as_bytes())
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
        Ok(FileBlob {
            path: blob.path,
            sha: blob.sha,
            bytes,
            encoding: blob.encoding,
            etag: None,
        })
    }

    async fn list_tags(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Tag>> {
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let tags: Vec<TagResponse> = self
            .get_json(&format!(
                "/repos/{}/{}/tags?per_page={per_page}&page={page}",
                reference.owner, reference.repo
            ))
            .await?;
        Ok(Page::single(
            tags.into_iter()
                .map(|tag| Tag {
                    name: tag.name,
                    sha: tag.commit.sha,
                    created_at: None,
                })
                .collect(),
        ))
    }

    async fn list_releases(
        &self,
        reference: &WorkspaceRef,
        opts: PageOpts,
    ) -> Result<Page<Release>> {
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let releases: Vec<ReleaseResponse> = self
            .get_json(&format!(
                "/repos/{}/{}/releases?per_page={per_page}&page={page}",
                reference.owner, reference.repo
            ))
            .await?;
        Ok(Page::single(
            releases
                .into_iter()
                .map(|release| Release {
                    id: release.id.to_string(),
                    tag_name: release.tag_name,
                    name: release.name,
                    prerelease: release.prerelease,
                    body: release.body,
                })
                .collect(),
        ))
    }

    async fn compare_refs(
        &self,
        reference: &WorkspaceRef,
        base: &GitRef,
        head: &GitRef,
    ) -> Result<RefComparison> {
        let comparison: CompareResponse = self
            .get_json(&format!(
                "/repos/{}/{}/compare/{}...{}",
                reference.owner,
                reference.repo,
                base.value(),
                head.value()
            ))
            .await?;
        Ok(RefComparison {
            status: comparison.status,
            ahead_by: comparison.ahead_by,
            behind_by: comparison.behind_by,
            files: comparison
                .files
                .unwrap_or_default()
                .into_iter()
                .map(|file| ChangedFile {
                    filename: file.filename,
                    status: file.status,
                    patch: file.patch,
                })
                .collect(),
        })
    }

    async fn list_members(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Member>> {
        let per_page = opts.per_page.unwrap_or(50);
        let page = opts.cursor.unwrap_or_else(|| "1".to_owned());
        let collaborators: Vec<CollaboratorResponse> = self
            .get_json(&format!(
                "/repos/{}/{}/collaborators?per_page={per_page}&page={page}&affiliation=all",
                reference.owner, reference.repo
            ))
            .await?;
        Ok(Page::single(
            collaborators.into_iter().map(Member::from).collect(),
        ))
    }

    async fn register_webhook(
        &self,
        reference: &WorkspaceRef,
        config: WebhookConfig,
    ) -> Result<WebhookHandle> {
        self.create_webhook(reference, config).await
    }

    async fn create_invitation(
        &self,
        reference: &WorkspaceRef,
        invite: InvitationInput,
    ) -> Result<Invitation> {
        self.invite_collaborator(reference, invite).await
    }

    async fn create_pull_request(
        &self,
        reference: &WorkspaceRef,
        input: PullRequestInput,
    ) -> Result<PullRequest> {
        let workspace = self.get_workspace(reference).await?;
        let base = input.base.unwrap_or(workspace.default_branch);
        let pr: PullRequestResponse = self
            .post_json(
                &format!("/repos/{}/{}/pulls", reference.owner, reference.repo),
                &CreatePullRequestRequest {
                    title: input.title,
                    head: input.branch_name,
                    base,
                    body: input.body,
                    draft: false,
                },
            )
            .await?;
        Ok(pr.into())
    }

    async fn merge_pull_request(
        &self,
        reference: &WorkspaceRef,
        number: u64,
    ) -> Result<PullRequest> {
        let merge: MergePullRequestResponse = self
            .put_json(
                &format!(
                    "/repos/{}/{}/pulls/{number}/merge",
                    reference.owner, reference.repo
                ),
                &MergePullRequestRequest {
                    merge_method: "squash",
                },
            )
            .await?;
        if !merge.merged {
            return Err(ProviderError::Conflict {
                resource: format!("pull request #{number}"),
                hint: merge.message,
            });
        }
        let pr: PullRequestResponse = self
            .get_json(&format!(
                "/repos/{}/{}/pulls/{number}",
                reference.owner, reference.repo
            ))
            .await?;
        Ok(pr.into())
    }

    async fn check_permission(
        &self,
        reference: &WorkspaceRef,
        login: &str,
    ) -> Result<PermissionLevel> {
        let permission: PermissionResponse = self
            .get_json(&format!(
                "/repos/{}/{}/collaborators/{}/permission",
                reference.owner, reference.repo, login
            ))
            .await?;
        Ok(match permission.permission.as_str() {
            "admin" => PermissionLevel::Admin,
            "maintain" => PermissionLevel::Maintain,
            "write" => PermissionLevel::Write,
            "triage" => PermissionLevel::Triage,
            "read" => PermissionLevel::Read,
            _ => PermissionLevel::None,
        })
    }
}
