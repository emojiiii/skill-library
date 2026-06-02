use async_trait::async_trait;
use base64::Engine;
use flate2::read::GzDecoder;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skill_library_core::WorkspaceRef;
use skill_library_provider::{
    ChangedFile, FileBlob, FileEntry, FileKind, GitRef, Invitation, InvitationInput, Member, Page,
    PageOpts, PermissionLevel, Provider, ProviderCapabilities, ProviderError, PullRequest,
    PullRequestInput, RefComparison, Release, Result, Tag, UnauthorizedReason, WebhookConfig,
    WebhookHandle, Workspace,
};
use std::path::{Component, Path, PathBuf};
use tracing;

pub mod scan;

pub struct GitHubProvider {
    client: reqwest::Client,
    api_base: String,
    authenticated: bool,
}

impl GitHubProvider {
    pub fn new(token: impl Into<String>) -> Result<Self> {
        Self::with_base_url("https://api.github.com", Some(token.into()))
    }

    pub fn anonymous(api_base: impl Into<String>) -> Result<Self> {
        Self::with_base_url(api_base, None)
    }

    pub fn with_base_url(api_base: impl Into<String>, token: Option<String>) -> Result<Self> {
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
            let snippet: String = String::from_utf8_lossy(&bytes).chars().take(200).collect();
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
            let snippet: String = String::from_utf8_lossy(&bytes).chars().take(200).collect();
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
            let message = response.text().await.unwrap_or_else(|_| status.to_string());
            return Err(provider_error_from_status(status, message));
        }

        let scopes = response
            .headers()
            .get("x-oauth-scopes")
            .and_then(|value| value.to_str().ok())
            .map(parse_scope_header)
            .unwrap_or_default();
        let user = response
            .json::<GitHubUser>()
            .await
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
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
            let message = response.text().await.unwrap_or_else(|_| status.to_string());
            return Err(provider_error_from_status(status, message));
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
            let existing = match self
                .read_file(
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
        let existing_sha = match self.read_file(reference, &git_ref, path).await {
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
        tracing::debug!(target: "skill-library-github", method = "PATCH", invitation_id);
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
        Err(provider_error_from_status(
            status,
            format!("PATCH /user/repository_invitations/{invitation_id} ({status}): {body}"),
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

// ---------------------------------------------------------------------------
// Pull requests, repository events, and user repository invitations.
// These power the Publish PRs / Activity / Invitations pages without an API server.
// ---------------------------------------------------------------------------

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
struct PullRequestListItemResponse {
    number: u64,
    title: String,
    html_url: String,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged_at: Option<String>,
    user: Option<OwnerResponse>,
    head: PullRequestRefResponse,
    base: PullRequestRefResponse,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PullRequestRefResponse {
    #[serde(default, rename = "ref")]
    ref_name: String,
    #[serde(default)]
    repo: Option<PullRequestRefRepoResponse>,
}

#[derive(Debug, Deserialize)]
struct PullRequestRefRepoResponse {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct PullRequestFileResponse {
    filename: String,
    status: String,
    #[serde(default)]
    patch: Option<String>,
}

#[derive(Debug, Serialize)]
struct ClosePullRequestRequest {
    state: &'static str,
}

#[derive(Debug, Serialize)]
struct IssueCommentRequest<'a> {
    body: &'a str,
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
struct RepositoryEventResponse {
    id: String,
    #[serde(rename = "type")]
    event_type: Option<String>,
    actor: Option<OwnerResponse>,
    created_at: String,
    #[serde(default)]
    payload: Option<serde_json::Value>,
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
struct CommitListItemResponse {
    sha: String,
    html_url: String,
    commit: CommitInnerResponse,
    #[serde(default)]
    author: Option<OwnerResponse>,
}

#[derive(Debug, Deserialize)]
struct CommitInnerResponse {
    message: String,
    author: CommitAuthorResponse,
}

#[derive(Debug, Deserialize)]
struct CommitAuthorResponse {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    date: String,
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

/// Minimal application/x-www-form-urlencoded escape for `path=` and `sha=`
/// query params (alphanum + - _ . ~ pass through, the rest become %XX).
fn urlencoding_simple(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

#[derive(Debug, Deserialize)]
struct RepositoryInvitationResponse {
    id: u64,
    repository: RepositoryInvitationRepoResponse,
    inviter: Option<OwnerResponse>,
    permissions: String,
    html_url: String,
    created_at: String,
    #[serde(default)]
    expired: bool,
}

#[derive(Debug, Deserialize)]
struct RepositoryInvitationRepoResponse {
    full_name: String,
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
struct DeviceCodeRequest<'a> {
    client_id: &'a str,
    scope: &'a str,
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
struct DeviceTokenRequest<'a> {
    client_id: &'a str,
    device_code: &'a str,
    grant_type: &'a str,
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

#[async_trait]
impl Provider for GitHubProvider {
    fn id(&self) -> &str {
        "github"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            webhook: true,
            release_assets: true,
            graphql: true,
            device_flow: true,
            refresh_token: false,
            bot_identity: true,
            pull_requests: true,
            invitations: true,
        }
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

async fn map_response<T: for<'de> Deserialize<'de>>(
    path: &str,
    method: &str,
    response: reqwest::Response,
) -> Result<T> {
    let status = response.status();
    if status.is_success() {
        let bytes = response
            .bytes()
            .await
            .map_err(|err| ProviderError::NetworkError {
                cause: err.to_string(),
            })?;
        return serde_json::from_slice::<T>(&bytes).map_err(|err| {
            let body = String::from_utf8_lossy(&bytes);
            let snippet: String = body.chars().take(200).collect();
            tracing::error!(
                target: "skill-library-github",
                method,
                path,
                status = status.as_u16(),
                body = %snippet,
                error = %err,
                "deserialize failed"
            );
            ProviderError::InvalidResponse(format!(
                "{method} {path} ({status}): deserialize failed: {err} — body: {snippet}"
            ))
        });
    }

    let message = response.text().await.unwrap_or_else(|_| status.to_string());
    let snippet: String = message.chars().take(200).collect();
    tracing::warn!(
        target: "skill-library-github",
        method,
        path,
        status = status.as_u16(),
        body = %snippet,
        "non-success response"
    );
    Err(provider_error_from_status(
        status,
        format!("{method} {path} ({status}): {snippet}"),
    ))
}

async fn map_empty_response(path: &str, method: &str, response: reqwest::Response) -> Result<()> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }

    let message = response.text().await.unwrap_or_else(|_| status.to_string());
    let snippet: String = message.chars().take(200).collect();
    tracing::warn!(
        target: "skill-library-github",
        method,
        path,
        status = status.as_u16(),
        body = %snippet,
        "non-success response"
    );
    Err(provider_error_from_status(
        status,
        format!("{method} {path} ({status}): {snippet}"),
    ))
}

fn provider_error_from_status(status: reqwest::StatusCode, message: String) -> ProviderError {
    match status.as_u16() {
        401 => ProviderError::Unauthorized {
            reason: UnauthorizedReason::TokenInvalid,
            missing_scopes: Vec::new(),
        },
        403 if message.to_ascii_lowercase().contains("rate limit") => ProviderError::RateLimited {
            retry_after_ms: 60_000,
            bucket: skill_library_provider::RateLimitBucket::Core,
        },
        429 => ProviderError::RateLimited {
            retry_after_ms: 60_000,
            bucket: skill_library_provider::RateLimitBucket::Core,
        },
        403 => ProviderError::Forbidden {
            resource: "github".to_owned(),
            reason: Some(message),
        },
        404 => ProviderError::NotFound {
            resource: message,
            reference: None,
        },
        409 | 422 => ProviderError::Conflict {
            resource: "github resource".to_owned(),
            hint: Some(message),
        },
        status if status >= 500 => ProviderError::ProviderUnavailable {
            status: Some(status),
            message,
        },
        _ => ProviderError::InvalidResponse(message),
    }
}

fn parse_scope_header(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(str::to_owned)
        .collect()
}

fn extract_tarball(bytes: &[u8], destination: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(destination)
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    let mut archive = tar::Archive::new(GzDecoder::new(bytes));
    let mut top_level: Option<PathBuf> = None;
    for entry in archive
        .entries()
        .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?
    {
        let mut entry = entry.map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
        let path = entry
            .path()
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?
            .to_path_buf();
        validate_archive_path(&path)?;
        // git's `tarball` archives begin with a `pax_global_header` pseudo-entry
        // (a PAX global extended header carrying the commit sha). It is NOT the
        // repository directory — skip it when detecting the real top-level dir,
        // otherwise `extracted_root` points at a non-existent `pax_global_header`
        // path and every later read fails with "os error 3" on Windows.
        let entry_type = entry.header().entry_type();
        let is_pax_header = entry_type.is_pax_global_extensions()
            || entry_type.is_pax_local_extensions()
            || path.as_os_str() == "pax_global_header";
        if top_level.is_none() && !is_pax_header {
            top_level = path.components().next().map(|component| {
                let mut root = PathBuf::new();
                root.push(component.as_os_str());
                root
            });
        }
        entry
            .unpack_in(destination)
            .map_err(|err| ProviderError::InvalidResponse(err.to_string()))?;
    }
    Ok(top_level
        .map(|root| destination.join(root))
        .unwrap_or_else(|| destination.to_path_buf()))
}

fn validate_archive_path(path: &Path) -> Result<()> {
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(ProviderError::InvalidResponse(format!(
                "archive path is unsafe: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn validate_repo_path(path: &str) -> Result<()> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() {
        return Err(ProviderError::InvalidResponse(
            "repo path cannot be empty".to_owned(),
        ));
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(ProviderError::InvalidResponse(format!(
                "repo path is unsafe: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn validate_branch_ref(branch: &str) -> Result<()> {
    if branch.trim().is_empty()
        || branch.starts_with('/')
        || branch.ends_with('/')
        || branch.contains("..")
        || branch.contains('\\')
        || branch.contains(' ')
        || branch.contains("refs/")
    {
        return Err(ProviderError::Conflict {
            resource: "branch".to_owned(),
            hint: Some(format!("unsafe branch ref: {branch}")),
        });
    }
    Ok(())
}

fn github_permission_role(role: &PermissionLevel) -> Result<&'static str> {
    match role {
        PermissionLevel::Admin => Ok("admin"),
        PermissionLevel::Maintain => Ok("maintain"),
        PermissionLevel::Write => Ok("push"),
        PermissionLevel::Triage => Ok("triage"),
        PermissionLevel::Read => Ok("pull"),
        PermissionLevel::None => Err(ProviderError::InvalidResponse(
            "invitation role cannot be none".to_owned(),
        )),
    }
}

fn github_webhook_request(config: WebhookConfig) -> Result<CreateWebhookRequest> {
    if config.callback_url.trim().is_empty() {
        return Err(ProviderError::InvalidResponse(
            "webhook callback url is required".to_owned(),
        ));
    }
    if config.secret.trim().is_empty() {
        return Err(ProviderError::InvalidResponse(
            "webhook secret is required".to_owned(),
        ));
    }
    let mut events = Vec::new();
    for event in config.events {
        let event = event.trim();
        if !event.is_empty() && !events.iter().any(|existing| existing == event) {
            events.push(event.to_owned());
        }
    }
    if events.is_empty() {
        events.push("push".to_owned());
    }

    Ok(CreateWebhookRequest {
        name: "web",
        active: true,
        events,
        config: CreateWebhookConfig {
            url: config.callback_url,
            content_type: "json",
            secret: config.secret,
            insecure_ssl: "0",
        },
    })
}

#[derive(Debug, Serialize)]
struct CreateWebhookRequest {
    name: &'static str,
    active: bool,
    events: Vec<String>,
    config: CreateWebhookConfig,
}

#[derive(Debug, Serialize)]
struct CreateWebhookConfig {
    url: String,
    content_type: &'static str,
    secret: String,
    insecure_ssl: &'static str,
}

#[derive(Debug, Serialize)]
struct CreateGitRefRequest {
    #[serde(rename = "ref")]
    reference: String,
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GitReferenceResponse {
    object: GitReferenceObject,
}

#[derive(Debug, Deserialize)]
struct GitReferenceObject {
    sha: String,
}

#[derive(Debug, Serialize)]
struct PutContentRequest {
    message: String,
    content: String,
    branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PutContentResponse {
    content: PutContentInfo,
}

#[derive(Debug, Deserialize)]
struct PutContentInfo {
    sha: String,
}

#[derive(Debug, Serialize)]
struct CreatePullRequestRequest {
    title: String,
    head: String,
    base: String,
    body: String,
    draft: bool,
}

#[derive(Debug, Deserialize)]
struct PullRequestResponse {
    number: u64,
    title: String,
    html_url: String,
    state: String,
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
struct MergePullRequestRequest {
    merge_method: &'static str,
}

#[derive(Debug, Deserialize)]
struct MergePullRequestResponse {
    merged: bool,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct CollaboratorInvitationRequest {
    permission: &'static str,
}

#[derive(Debug, Deserialize)]
struct CollaboratorInvitationResponse {
    id: u64,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    invitee: Option<InviteeResponse>,
}

#[derive(Debug, Deserialize)]
struct InviteeResponse {
    login: String,
}

#[derive(Debug, Deserialize)]
struct WebhookResponse {
    id: u64,
    #[serde(default)]
    config: Option<WebhookResponseConfig>,
}

#[derive(Debug, Deserialize)]
struct WebhookResponseConfig {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoResponse {
    name: String,
    full_name: String,
    default_branch: String,
    private: bool,
    html_url: Option<String>,
    owner: OwnerResponse,
    permissions: Option<RepoPermissions>,
}

impl From<RepoResponse> for Workspace {
    fn from(repo: RepoResponse) -> Self {
        let permission = repo
            .permissions
            .map(PermissionLevel::from)
            .unwrap_or(PermissionLevel::None);
        Workspace {
            provider: "github".to_owned(),
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
struct OwnerResponse {
    login: String,
}

#[derive(Debug, Deserialize)]
struct RepoPermissions {
    admin: bool,
    #[serde(default)]
    maintain: bool,
    push: bool,
    #[serde(default)]
    triage: bool,
    pull: bool,
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
struct CollaboratorResponse {
    login: String,
    #[serde(default)]
    avatar_url: Option<String>,
    #[serde(default)]
    permissions: Option<RepoPermissions>,
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
struct TreeResponse {
    tree: Vec<TreeEntryResponse>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct TreeEntryResponse {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    sha: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ContentResponse {
    path: String,
    sha: String,
    content: String,
    encoding: String,
}

#[derive(Debug, Deserialize)]
struct TagResponse {
    name: String,
    commit: TagCommitResponse,
}

#[derive(Debug, Deserialize)]
struct TagCommitResponse {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    id: u64,
    tag_name: String,
    name: Option<String>,
    prerelease: bool,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompareResponse {
    status: String,
    ahead_by: u32,
    behind_by: u32,
    files: Option<Vec<CompareFileResponse>>,
}

#[derive(Debug, Deserialize)]
struct CompareFileResponse {
    filename: String,
    status: String,
    patch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PermissionResponse {
    permission: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_webhook_request_defaults_to_push_and_dedupes_events() {
        let request = github_webhook_request(WebhookConfig {
            events: vec!["push".to_owned(), "release".to_owned(), "push".to_owned()],
            callback_url: "https://team.example/api/webhooks/github".to_owned(),
            secret: "secret".to_owned(),
        })
        .unwrap();
        let value = serde_json::to_value(request).unwrap();

        assert_eq!(value["name"], "web");
        assert_eq!(value["active"], true);
        assert_eq!(value["events"], serde_json::json!(["push", "release"]));
        assert_eq!(
            value["config"],
            serde_json::json!({
                "url": "https://team.example/api/webhooks/github",
                "content_type": "json",
                "secret": "secret",
                "insecure_ssl": "0"
            })
        );

        let defaulted = github_webhook_request(WebhookConfig {
            events: Vec::new(),
            callback_url: "https://team.example/api/webhooks/github".to_owned(),
            secret: "secret".to_owned(),
        })
        .unwrap();
        assert_eq!(defaulted.events, vec!["push"]);
    }

    #[test]
    fn github_webhook_request_requires_callback_and_secret() {
        assert!(github_webhook_request(WebhookConfig {
            events: Vec::new(),
            callback_url: String::new(),
            secret: "secret".to_owned(),
        })
        .is_err());
        assert!(github_webhook_request(WebhookConfig {
            events: Vec::new(),
            callback_url: "https://team.example/api/webhooks/github".to_owned(),
            secret: String::new(),
        })
        .is_err());
    }

    #[test]
    fn extract_tarball_skips_pax_global_header_for_top_level() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        // Build a gzip tarball shaped like GitHub's `tarball` archives: a leading
        // `pax_global_header` pseudo-entry (PAX global extended header carrying the
        // commit sha), followed by the real `<owner>-<repo>-<sha>/` tree. The bug
        // was treating that first pseudo-entry as the top-level dir, so
        // `extracted_root` pointed at a non-existent `pax_global_header` path and
        // every later read failed with os error 3 on Windows.
        let mut tar_buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_buf);

            // pax_global_header pseudo-entry.
            let pax_payload = b"52 comment=387020e0000000000000000000000000000000\n";
            let mut pax_header = tar::Header::new_ustar();
            pax_header.set_size(pax_payload.len() as u64);
            pax_header.set_entry_type(tar::EntryType::new(b'g'));
            pax_header.set_cksum();
            builder
                .append_data(&mut pax_header, "pax_global_header", &pax_payload[..])
                .unwrap();

            // Real repository tree under owner-repo-sha/.
            let skill_md = b"---\nid: nested-skill\ntype: skill\nname: Nested\ndescription: A nested skill.\nversion: 0.1.0\ntargets:\n  - claude-code\n---\n# Nested\n";
            let mut md_header = tar::Header::new_ustar();
            md_header.set_size(skill_md.len() as u64);
            md_header.set_entry_type(tar::EntryType::Regular);
            md_header.set_mode(0o644);
            md_header.set_cksum();
            builder
                .append_data(
                    &mut md_header,
                    "owner-repo-387020e/skills/cat/nested-skill/SKILL.md",
                    &skill_md[..],
                )
                .unwrap();
            builder.finish().unwrap();
        }

        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(&tar_buf).unwrap();
        let gz_bytes = gz.finish().unwrap();

        let dest = tempfile::tempdir().unwrap();
        let extracted_root = extract_tarball(&gz_bytes, dest.path()).unwrap();

        // Must point at the real tree, not pax_global_header, and exist on disk.
        assert_eq!(
            extracted_root.file_name().and_then(|n| n.to_str()),
            Some("owner-repo-387020e"),
            "extracted_root should be the real repo dir, got {extracted_root:?}"
        );
        assert!(
            extracted_root.is_dir(),
            "extracted_root must exist as a dir"
        );
        assert!(
            extracted_root
                .join("skills/cat/nested-skill/SKILL.md")
                .exists(),
            "nested skill must be readable under extracted_root"
        );
    }

    #[test]
    fn collaborator_response_maps_highest_permission_to_member_role() {
        let collaborator: CollaboratorResponse = serde_json::from_value(serde_json::json!({
            "login": "octocat",
            "avatar_url": "https://avatars.githubusercontent.com/u/1?v=4",
            "permissions": {
                "admin": false,
                "maintain": true,
                "push": true,
                "triage": true,
                "pull": true
            }
        }))
        .unwrap();

        let member = Member::from(collaborator);

        assert_eq!(member.login, "octocat");
        assert_eq!(member.role, PermissionLevel::Maintain);
        assert_eq!(
            member.avatar_url.as_deref(),
            Some("https://avatars.githubusercontent.com/u/1?v=4")
        );
    }

    #[test]
    fn collaborator_response_without_permissions_maps_to_none() {
        let collaborator: CollaboratorResponse =
            serde_json::from_value(serde_json::json!({ "login": "outside-user" })).unwrap();

        let member = Member::from(collaborator);

        assert_eq!(member.login, "outside-user");
        assert_eq!(member.role, PermissionLevel::None);
        assert_eq!(member.avatar_url, None);
    }

    /// Regression test for the "sync_error: io error: json error: missing field `id`" bug.
    /// When GitHub returns a 200 body that doesn't match the expected shape, the resulting
    /// error should carry enough context (HTTP method, path, status, body snippet) for the
    /// caller to figure out which GitHub endpoint is failing.
    #[tokio::test]
    async fn deserialize_failure_includes_endpoint_context() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/repos/acme/team-skills/hooks")
            .with_status(200)
            .with_header("content-type", "application/json")
            // Body missing the required `id` field — this used to surface as
            // a bare `missing field 'id'` with no hint of which call failed.
            .with_body(r#"{"config":{"url":"https://example.com"}}"#)
            .create_async()
            .await;

        let provider = GitHubProvider::anonymous(server.url()).unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");

        let result = provider
            .create_webhook(
                &workspace,
                WebhookConfig {
                    events: vec!["push".to_owned()],
                    callback_url: "https://example.com/hook".to_owned(),
                    secret: "shh".to_owned(),
                },
            )
            .await;

        mock.assert_async().await;
        let err = result.expect_err("expected deserialize failure to surface as ProviderError");
        let message = err.to_string();
        assert!(
            message.contains("POST"),
            "error should mention the HTTP method, got: {message}"
        );
        assert!(
            message.contains("/repos/acme/team-skills/hooks"),
            "error should mention the failing path, got: {message}"
        );
        assert!(
            message.contains("missing field"),
            "error should still expose the underlying serde reason, got: {message}"
        );
    }

    /// When GitHub returns a non-2xx error body, we should classify it (Forbidden / NotFound /
    /// Conflict / etc.) instead of letting the body fall into a generic deserialize failure.
    #[tokio::test]
    async fn forbidden_response_maps_to_forbidden_error_with_path() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/repos/acme/team-skills/hooks")
            .with_status(403)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"Resource not accessible by integration"}"#)
            .create_async()
            .await;

        let provider = GitHubProvider::anonymous(server.url()).unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");

        let result = provider
            .create_webhook(
                &workspace,
                WebhookConfig {
                    events: vec!["push".to_owned()],
                    callback_url: "https://example.com/hook".to_owned(),
                    secret: "shh".to_owned(),
                },
            )
            .await;

        mock.assert_async().await;
        let err = result.expect_err("expected 403 to surface as Forbidden");
        assert!(
            matches!(err, ProviderError::Forbidden { .. }),
            "expected Forbidden, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn github_rate_limit_response_maps_to_rate_limited() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/repos/acme/team-skills/hooks")
            .with_status(403)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"API rate limit exceeded for 203.0.113.1."}"#)
            .create_async()
            .await;

        let provider = GitHubProvider::anonymous(server.url()).unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");

        let result = provider
            .create_webhook(
                &workspace,
                WebhookConfig {
                    events: vec!["push".to_owned()],
                    callback_url: "https://example.com/hook".to_owned(),
                    secret: "shh".to_owned(),
                },
            )
            .await;

        mock.assert_async().await;
        let err = result.expect_err("expected GitHub API rate limit to surface as RateLimited");
        assert!(
            matches!(err, ProviderError::RateLimited { .. }),
            "expected RateLimited, got: {err:?}"
        );
    }

    /// Scanning a workspace for skills should hit GitHub at most twice:
    ///   1. Trees API (recursive=1) to enumerate manifest paths
    ///   2. One GraphQL batch to pull every manifest's text
    /// Regardless of how many skills the repo has.
    #[tokio::test]
    async fn scan_uses_two_calls_for_any_skill_count() {
        use crate::scan::scan_skill_assets_at;

        let mut server = mockito::Server::new_async().await;

        // Tree response with 3 skills nested at different depths to ensure the
        // scanner finds them all in one pass.
        let tree_mock = server
            .mock(
                "GET",
                "/repos/acme/team-skills/git/trees/main?recursive=1",
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "tree": [
                        {"path": "skills/code-reviewer/manifest.yaml", "type": "blob", "sha": "a"},
                        {"path": "skills/pr-summarizer/SKILL.md", "type": "blob", "sha": "b"},
                        {"path": "deep/nested/skills/security-auditor/manifest.json", "type": "blob", "sha": "c"},
                        {"path": "README.md", "type": "blob", "sha": "d"},
                        {"path": ".github/workflows/ci.yml", "type": "blob", "sha": "e"}
                    ]
                }"#,
            )
            // The whole point: only ONE tree call.
            .expect(1)
            .create_async()
            .await;

        // GraphQL response with manifests for all 3 skills, returned as
        // f0/f1/f2 aliases. The scanner iterates skill_dirs in alphabetical
        // order (BTreeMap), so:
        //   f0 = deep/nested/skills/security-auditor (manifest.json)
        //   f1 = skills/code-reviewer (manifest.yaml)
        //   f2 = skills/pr-summarizer (SKILL.md)
        let graphql_mock = server
            .mock("POST", "/graphql")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "data": {
                        "repository": {
                            "f0": {"text": "{\"schemaVersion\":1,\"id\":\"security-auditor\",\"type\":\"skill\",\"name\":\"Auditor\",\"description\":\"Audit\",\"version\":\"2.0.0\",\"targets\":[\"claude-code\"]}", "isBinary": false, "byteSize": 150},
                            "f1": {"text": "schemaVersion: 1\nid: code-reviewer\ntype: skill\nname: Code Reviewer\ndescription: Reviews code\nversion: 1.0.0\ntargets: [claude-code]", "isBinary": false, "byteSize": 100},
                            "f2": {"text": "---\nschemaVersion: 1\nid: pr-summarizer\ntype: skill\nname: PR Summarizer\ndescription: Summarizes PRs\nversion: 0.1.0\ntargets: [claude-code]\n---\n# PR Summarizer\n", "isBinary": false, "byteSize": 200}
                        }
                    }
                }"#,
            )
            // And only ONE GraphQL call regardless of skill count.
            .expect(1)
            .create_async()
            .await;

        let provider = GitHubProvider::anonymous(server.url()).unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");
        let skills =
            scan_skill_assets_at(&provider, &workspace, &GitRef::Branch("main".to_owned()))
                .await
                .expect("scan succeeded");

        tree_mock.assert_async().await;
        graphql_mock.assert_async().await;

        let ids: Vec<&str> = skills.iter().map(|s| s.manifest.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["code-reviewer", "pr-summarizer", "security-auditor"]
        );
    }

    /// One bad manifest must not poison the whole workspace — the rest of the
    /// skills should still come back, with the bad one logged and skipped.
    #[tokio::test]
    async fn scan_skips_individual_bad_manifests() {
        use crate::scan::scan_skill_assets_at;

        let mut server = mockito::Server::new_async().await;

        let _tree = server
            .mock("GET", "/repos/acme/team-skills/git/trees/main?recursive=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "tree": [
                        {"path": "good/manifest.yaml", "type": "blob", "sha": "a"},
                        {"path": "bad/manifest.yaml", "type": "blob", "sha": "b"}
                    ]
                }"#,
            )
            .create_async()
            .await;

        // Second manifest is missing required fields (no `id`). Old behavior:
        // crash whole scan. New behavior: skip and continue.
        let _gql = server
            .mock("POST", "/graphql")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "data": {
                        "repository": {
                            "f0": {"text": "schemaVersion: 1\nid: good-skill\ntype: skill\nname: Good\ndescription: ok\nversion: 1.0.0\ntargets: [claude-code]", "isBinary": false, "byteSize": 100},
                            "f1": {"text": "name: Missing required fields\nversion: 0.1.0", "isBinary": false, "byteSize": 50}
                        }
                    }
                }"#,
            )
            .create_async()
            .await;

        let provider = GitHubProvider::anonymous(server.url()).unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");
        let skills =
            scan_skill_assets_at(&provider, &workspace, &GitRef::Branch("main".to_owned()))
                .await
                .expect("scan succeeded despite one bad manifest");

        let ids: Vec<&str> = skills.iter().map(|s| s.manifest.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["good-skill"],
            "bad manifest must be skipped, not crash the scan"
        );
    }

    /// Empty repo (no manifests at all) is a valid result — empty skill list,
    /// no error, and we shouldn't make a GraphQL call when there's nothing to fetch.
    #[tokio::test]
    async fn scan_empty_workspace_skips_graphql() {
        use crate::scan::scan_skill_assets_at;

        let mut server = mockito::Server::new_async().await;

        let _tree = server
            .mock("GET", "/repos/acme/team-skills/git/trees/main?recursive=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{ "tree": [{"path": "README.md", "type": "blob", "sha": "a"}] }"#)
            .create_async()
            .await;

        // Mock GraphQL but expect ZERO calls — scan should bail out before
        // hitting GraphQL when there are no skill candidates.
        let no_graphql = server
            .mock("POST", "/graphql")
            .expect(0)
            .create_async()
            .await;

        let provider = GitHubProvider::anonymous(server.url()).unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");
        let skills =
            scan_skill_assets_at(&provider, &workspace, &GitRef::Branch("main".to_owned()))
                .await
                .expect("empty scan should succeed");

        no_graphql.assert_async().await;
        assert!(skills.is_empty());
    }
}
