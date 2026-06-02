mod ai_review;
mod app_icons;
mod db;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use skill_library_core::{AppPaths, GitHubCredential, WorkspaceRef};
use skill_library_installer::{InstallMetadata, InstallOptions, InstallReport, TargetRoot};
use skill_library_manifest::{effective_risk, SemanticChange, SkillAsset};
use skill_library_provider::{
    ChangedFile, GitRef, Invitation, InvitationInput, Member, PageOpts, PermissionLevel, Provider,
    Workspace,
};
use skill_library_provider_github::{
    scan::{SkillDetailScan, WorkspaceDetailScan},
    CommitSummary, GitHubProvider, GitHubPublishFile, GitHubPublishInput, IssueComment,
    PullRequestQueryState, PullRequestSummary, RepositoryEvent, RepositoryInvitation,
};
use skill_library_publish::{PublishPackage, PublishPolicyResult, PublishRequestSummary};
use skill_library_sync::{
    RemoteWorkspaceSkills, StoredWorkspace, Subscription, SubscriptionsFile, SyncOptions,
    SyncReport, TargetSelection, WorkspaceWebhookRegistration, WorkspacesFile,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use url::Url;

const DEEP_LINK_EVENT: &str = "skill-library://deep-link";
const DEEP_LINK_SUBSCRIBE_PATH: &str = "subscribe";
const GITHUB_DEVICE_SCOPES: &[&str] = &["repo", "read:org", "read:user"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeepLinkPayload {
    url: String,
    action: String,
    workspace: Option<WorkspaceRef>,
    asset_id: Option<String>,
    version: Option<String>,
    targets: Vec<String>,
    query: HashMap<String, String>,
}

#[derive(Default)]
struct DeepLinkState {
    last: Mutex<Option<DeepLinkPayload>>,
}

#[derive(Debug, thiserror::Error)]
enum CommandError {
    #[error("{message}")]
    Coded { code: &'static str, message: String },
}

impl CommandError {
    fn coded(code: &'static str, message: impl Into<String>) -> Self {
        Self::Coded {
            code,
            message: message.into(),
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::Coded { code, .. } => code,
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::Coded { message, .. } => message,
        }
    }
}

impl serde::Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct CommandErrorBody<'a> {
            code: &'static str,
            message: &'a str,
        }

        CommandErrorBody {
            code: self.code(),
            message: self.message(),
        }
        .serialize(serializer)
    }
}

impl From<skill_library_core::SkillLibraryError> for CommandError {
    fn from(value: skill_library_core::SkillLibraryError) -> Self {
        Self::coded("core_error", value.to_string())
    }
}

impl From<skill_library_manifest::ManifestError> for CommandError {
    fn from(value: skill_library_manifest::ManifestError) -> Self {
        Self::coded("manifest_error", value.to_string())
    }
}

impl From<skill_library_installer::InstallerError> for CommandError {
    fn from(value: skill_library_installer::InstallerError) -> Self {
        Self::coded("installer_error", value.to_string())
    }
}

impl From<skill_library_sync::SyncError> for CommandError {
    fn from(value: skill_library_sync::SyncError) -> Self {
        let code = match value {
            skill_library_sync::SyncError::NotFound(_) => "subscription_not_found",
            skill_library_sync::SyncError::RemoteNotFound(_) => "remote_not_found",
            _ => "sync_error",
        };
        Self::coded(code, value.to_string())
    }
}

impl From<skill_library_publish::PublishError> for CommandError {
    fn from(value: skill_library_publish::PublishError) -> Self {
        Self::coded("publish_error", value.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(value: std::io::Error) -> Self {
        Self::coded("io_error", value.to_string())
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(value: serde_json::Error) -> Self {
        Self::coded("json_error", value.to_string())
    }
}

type CommandResult<T> = Result<T, CommandError>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStateSummary {
    home: PathBuf,
    config: PathBuf,
    subscriptions: PathBuf,
    workspaces: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishPreview {
    package: PublishPackage,
    policy: PublishPolicyResult,
    request: Option<PublishRequestSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishResult {
    package: PublishPackage,
    policy: PublishPolicyResult,
    request: PublishRequestSummary,
    pull_request: PublishPullRequestSummary,
    target_workspace: String,
    uploaded_files: Vec<String>,
    auto_merge: Option<PublishAutoMergeResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishPullRequestSummary {
    number: u64,
    title: String,
    html_url: String,
    state: String,
}

impl From<skill_library_provider::PullRequest> for PublishPullRequestSummary {
    fn from(value: skill_library_provider::PullRequest) -> Self {
        Self {
            number: value.number,
            title: value.title,
            html_url: value.html_url,
            state: value.state,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishAutoMergeResult {
    merged: bool,
    deleted_branch: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishDraftInput {
    file_path: String,
    after: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsExport {
    exported_at: String,
    output_dir: PathBuf,
    app_home: PathBuf,
    subscriptions: usize,
    workspaces: usize,
    logs: Vec<PathBuf>,
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledTargetGroup {
    target: String,
    skills: Vec<InstallMetadata>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalAgentRoot {
    id: String,
    label: String,
    kind: String,
    path: PathBuf,
    exists: bool,
    entries: Vec<LocalAgentEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalAgentEntry {
    id: String,
    name: String,
    path: PathBuf,
    has_manifest: bool,
    has_skill_md: bool,
    managed: bool,
    version: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
struct WorkspaceSkillsResponse {
    workspace: Workspace,
    skills: Vec<SkillAsset>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    github_login: Option<String>,
    github_scopes: Vec<String>,
    credential_store: String,
    warning: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubLoginResult {
    login: String,
    scopes: Vec<String>,
    credential_store: String,
    warning: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubDeviceStartResult {
    client_id: String,
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_at: u64,
    interval: u64,
    scopes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum GitHubDevicePollResult {
    Pending,
    SlowDown { interval: u64 },
    Authorized { login: GitHubLoginResult },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillComparison {
    workspace: WorkspaceRef,
    skill_path: String,
    from: String,
    to: String,
    files: Vec<ChangedFile>,
    semantic: Vec<SemanticChange>,
}

#[tauri::command]
fn app_init() -> CommandResult<AppStateSummary> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    Ok(AppStateSummary {
        home: paths.home,
        config: paths.config,
        subscriptions: paths.subscriptions,
        workspaces: paths.workspaces,
    })
}

#[tauri::command]
fn get_deep_link_state(app: tauri::AppHandle) -> CommandResult<Option<DeepLinkPayload>> {
    Ok(app.state::<DeepLinkState>().last.lock().unwrap().clone())
}

#[tauri::command]
fn scan_workspace(path: String) -> CommandResult<Vec<SkillAsset>> {
    Ok(skill_library_manifest::scan_workspace(path)?)
}

#[tauri::command]
fn get_auth_status() -> CommandResult<AuthStatus> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let github = skill_library_core::load_github_credential(&paths.credentials)?;
    let credential_store = skill_library_core::keychain_store_name().to_owned();
    let (github_login, github_scopes, warning) = match github {
        Some(github) => (
            github.login,
            github.scopes,
            Some(credential_warning(&paths, &credential_store)),
        ),
        None => (None, Vec::new(), None),
    };
    Ok(AuthStatus {
        github_login,
        github_scopes,
        credential_store: credential_store.clone(),
        warning,
    })
}

fn credential_warning(paths: &AppPaths, credential_store: &str) -> String {
    match credential_store {
        "os-keychain" => format!(
            "GitHub token is stored in the OS keychain; {} keeps only non-sensitive login metadata.",
            paths.credentials.display()
        ),
        _ => format!(
            "GitHub token is stored in the active keyring backend; {} keeps only non-sensitive login metadata.",
            paths.credentials.display()
        ),
    }
}

#[tauri::command]
async fn login_github_token(token: String) -> CommandResult<GitHubLoginResult> {
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err(CommandError::coded(
            "missing_github_token",
            "GitHub token is required",
        ));
    }

    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let provider = GitHubProvider::new(token.clone()).map_err(provider_command_error)?;
    let info = provider
        .validate_token()
        .await
        .map_err(provider_command_error)?;
    skill_library_core::save_github_credential(
        &paths.credentials,
        GitHubCredential {
            token,
            login: Some(info.user.login.clone()),
            scopes: info.scopes.clone(),
        },
    )?;
    let credential_store = skill_library_core::keychain_store_name().to_owned();

    Ok(GitHubLoginResult {
        login: info.user.login,
        scopes: info.scopes,
        credential_store: credential_store.clone(),
        warning: credential_warning(&paths, &credential_store),
    })
}

#[tauri::command]
fn logout_github() -> CommandResult<()> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    skill_library_core::delete_github_credential(&paths.credentials)?;
    Ok(())
}

#[tauri::command]
async fn start_github_device_flow(
    client_id: Option<String>,
) -> CommandResult<GitHubDeviceStartResult> {
    let client_id = resolve_github_client_id(client_id)?;
    let device = GitHubProvider::start_device_flow(&client_id, GITHUB_DEVICE_SCOPES)
        .await
        .map_err(provider_command_error)?;
    let expires_at = current_unix_secs().saturating_add(device.expires_in);
    Ok(GitHubDeviceStartResult {
        client_id,
        device_code: device.device_code,
        user_code: device.user_code,
        verification_uri: device.verification_uri,
        verification_uri_complete: device.verification_uri_complete,
        expires_at,
        interval: device.interval.max(1),
        scopes: GITHUB_DEVICE_SCOPES
            .iter()
            .map(|scope| (*scope).to_owned())
            .collect(),
    })
}

#[tauri::command]
async fn poll_github_device_flow(
    client_id: String,
    device_code: String,
) -> CommandResult<GitHubDevicePollResult> {
    let client_id = client_id.trim().to_owned();
    let device_code = device_code.trim().to_owned();
    if client_id.is_empty() || device_code.is_empty() {
        return Err(CommandError::coded(
            "missing_device_session",
            "GitHub device flow session is missing",
        ));
    }

    let response = GitHubProvider::poll_device_flow(&client_id, &device_code)
        .await
        .map_err(provider_command_error)?;
    if let Some(token) = response.access_token {
        let paths = AppPaths::resolve()?;
        skill_library_sync::ensure_local_state(&paths)?;
        let provider = GitHubProvider::new(token.clone()).map_err(provider_command_error)?;
        let info = provider
            .validate_token()
            .await
            .map_err(provider_command_error)?;
        skill_library_core::save_github_credential(
            &paths.credentials,
            GitHubCredential {
                token,
                login: Some(info.user.login.clone()),
                scopes: info.scopes.clone(),
            },
        )?;
        let credential_store = skill_library_core::keychain_store_name().to_owned();
        return Ok(GitHubDevicePollResult::Authorized {
            login: GitHubLoginResult {
                login: info.user.login,
                scopes: info.scopes,
                credential_store: credential_store.clone(),
                warning: credential_warning(&paths, &credential_store),
            },
        });
    }

    match response.error.as_deref() {
        Some("authorization_pending") | None => Ok(GitHubDevicePollResult::Pending),
        Some("slow_down") => Ok(GitHubDevicePollResult::SlowDown { interval: 5 }),
        Some("expired_token") => Err(CommandError::coded(
            "github_device_expired",
            "GitHub device code expired",
        )),
        Some("access_denied") => Err(CommandError::coded(
            "github_device_access_denied",
            "GitHub device authorization was cancelled",
        )),
        Some(error) => Err(CommandError::coded(
            "github_device_error",
            response
                .error_description
                .unwrap_or_else(|| error.to_owned()),
        )),
    }
}

#[tauri::command]
fn list_workspaces() -> CommandResult<WorkspacesFile> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    Ok(skill_library_sync::read_workspaces(
        &paths.workspace_registry,
    )?)
}

#[tauri::command]
async fn add_workspace(
    workspace: String,
    token: Option<String>,
    webhook_url: Option<String>,
    webhook_secret: Option<String>,
    webhook_events: Option<Vec<String>>,
) -> CommandResult<StoredWorkspace> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = token.or_else(|| saved_github_token(&paths));
    let webhook = workspace_webhook_registration(webhook_url, webhook_secret, webhook_events)?;
    Ok(skill_library_sync::add_github_workspace_with_webhook(
        &paths.workspace_registry,
        &workspace,
        token.as_deref(),
        webhook,
    )
    .await?)
}

#[tauri::command]
async fn scan_github_workspace(
    workspace: String,
    token: Option<String>,
) -> CommandResult<WorkspaceSkillsResponse> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = token.or_else(|| saved_github_token(&paths));
    let remote: RemoteWorkspaceSkills =
        skill_library_sync::scan_github_workspace_skills(&workspace, token.as_deref()).await?;
    Ok(WorkspaceSkillsResponse {
        workspace: remote.workspace,
        skills: remote.skills,
    })
}

#[tauri::command]
async fn scan_github_workspace_streaming(
    app: tauri::AppHandle,
    workspace: String,
    token: Option<String>,
) -> CommandResult<WorkspaceSkillsResponse> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = token.or_else(|| saved_github_token(&paths));
    let app_handle = app.clone();
    let remote: RemoteWorkspaceSkills = skill_library_sync::scan_github_workspace_skills_streaming(
        &workspace,
        token.as_deref(),
        |batch| {
            let _ = app_handle.emit("workspace-scan-progress", batch);
        },
    )
    .await?;
    Ok(WorkspaceSkillsResponse {
        workspace: remote.workspace,
        skills: remote.skills,
    })
}

#[tauri::command]
async fn get_workspace_detail(
    workspace: String,
    token: Option<String>,
) -> CommandResult<WorkspaceDetailScan> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = token.or_else(|| saved_github_token(&paths));
    Ok(skill_library_sync::scan_github_workspace_detail(&workspace, token.as_deref()).await?)
}

#[tauri::command]
async fn get_skill_detail(
    workspace: String,
    skill_path: String,
    ref_name: Option<String>,
    token: Option<String>,
) -> CommandResult<SkillDetailScan> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = token.or_else(|| saved_github_token(&paths));
    Ok(skill_library_sync::read_github_skill_detail(
        &workspace,
        &skill_path,
        ref_name.as_deref(),
        token.as_deref(),
    )
    .await?)
}

// ---------------------------------------------------------------------------
// Public skill registry (skills.sh) — consumer-layer discovery.
//
// Anonymous, no GitHub token required. The desktop webview cannot fetch
// skills.sh directly (no CORS headers), so this command proxies the request
// server-side and caches results briefly to avoid hammering the registry.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistrySkill {
    /// Composite id: "owner/repo/skillId"
    id: String,
    skill_id: String,
    name: String,
    #[serde(default)]
    installs: u64,
    /// GitHub "owner/repo" — feeds the anonymous read path (get_skill_detail).
    source: String,
    #[serde(default)]
    is_official: bool,
}

#[derive(serde::Deserialize)]
struct RegistrySearchResponse {
    #[serde(default)]
    skills: Vec<RegistrySkill>,
}

struct RegistryCacheEntry {
    fetched_at: SystemTime,
    skills: Vec<RegistrySkill>,
}

#[derive(Default)]
struct RegistryCache {
    entries: Mutex<HashMap<String, RegistryCacheEntry>>,
}

const REGISTRY_CACHE_TTL_SECS: u64 = 600; // 10 minutes
const REGISTRY_SEARCH_URL: &str = "https://skills.sh/api/search";

#[tauri::command]
async fn search_skills_registry(
    cache: tauri::State<'_, RegistryCache>,
    query: String,
) -> CommandResult<Vec<RegistrySkill>> {
    let needle = query.trim().to_string();
    if needle.len() < 2 {
        return Ok(Vec::new());
    }
    let cache_key = needle.to_lowercase();

    // Serve from cache when fresh.
    if let Ok(entries) = cache.entries.lock() {
        if let Some(entry) = entries.get(&cache_key) {
            if entry
                .fetched_at
                .elapsed()
                .map(|age| age.as_secs() < REGISTRY_CACHE_TTL_SECS)
                .unwrap_or(false)
            {
                return Ok(entry.skills.clone());
            }
        }
    }

    let client = reqwest::Client::builder()
        .user_agent("skill-library/0.1")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|err| CommandError::coded("registry_client", err.to_string()))?;

    let response = client
        .get(REGISTRY_SEARCH_URL)
        .query(&[("q", needle.as_str())])
        .send()
        .await
        .map_err(|err| CommandError::coded("registry_request", err.to_string()))?;

    if !response.status().is_success() {
        return Err(CommandError::coded(
            "registry_status",
            format!("skill registry returned HTTP {}", response.status()),
        ));
    }

    let parsed: RegistrySearchResponse = response
        .json()
        .await
        .map_err(|err| CommandError::coded("registry_parse", err.to_string()))?;

    if let Ok(mut entries) = cache.entries.lock() {
        entries.insert(
            cache_key,
            RegistryCacheEntry {
                fetched_at: SystemTime::now(),
                skills: parsed.skills.clone(),
            },
        );
    }

    Ok(parsed.skills)
}

#[tauri::command]
async fn list_github_workspaces(query: Option<String>) -> CommandResult<Vec<Workspace>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with a GitHub token before listing repositories",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let page = provider
        .list_workspaces(PageOpts {
            cursor: None,
            per_page: Some(100),
        })
        .await
        .map_err(provider_command_error)?;
    let needle = query.unwrap_or_default().trim().to_lowercase();
    let mut workspaces: Vec<Workspace> = page
        .items
        .into_iter()
        .filter(|workspace| {
            needle.is_empty() || workspace.full_name.to_lowercase().contains(&needle)
        })
        .collect();
    workspaces.sort_by(|a, b| a.full_name.cmp(&b.full_name));
    Ok(workspaces)
}

#[tauri::command]
async fn invite_github_collaborator(
    workspace: String,
    login: String,
    role: String,
    token: Option<String>,
) -> CommandResult<Invitation> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = token
        .or_else(|| saved_github_token(&paths))
        .ok_or_else(|| {
            CommandError::coded(
                "missing_github_token",
                "GitHub token is required for invite",
            )
        })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let current_user = provider
        .current_user()
        .await
        .map_err(provider_command_error)?;
    let permission = provider
        .check_permission(&workspace, &current_user.login)
        .await
        .map_err(provider_command_error)?;
    if !matches!(
        permission,
        PermissionLevel::Admin | PermissionLevel::Maintain
    ) {
        return Err(CommandError::coded(
            "insufficient_permission",
            format!(
                "github user {} must have admin or maintain permission on {} to invite collaborators",
                current_user.login,
                workspace.full_name()
            ),
        ));
    }
    provider
        .create_invitation(
            &workspace,
            InvitationInput {
                login_or_email: login,
                role: parse_permission_role(&role)?,
            },
        )
        .await
        .map_err(provider_command_error)
}

#[tauri::command]
async fn list_workspace_members(
    workspace: String,
    token: Option<String>,
) -> CommandResult<Vec<Member>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = token
        .or_else(|| saved_github_token(&paths))
        .ok_or_else(|| {
            CommandError::coded(
                "missing_github_token",
                "GitHub token is required for listing workspace members",
            )
        })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let members = provider
        .list_members(
            &workspace,
            PageOpts {
                cursor: None,
                per_page: Some(100),
            },
        )
        .await
        .map_err(provider_command_error)?;
    Ok(members.items)
}

#[tauri::command]
async fn compare_skill_versions(
    workspace: String,
    skill_path: String,
    from: String,
    to: String,
    token: Option<String>,
) -> CommandResult<SkillComparison> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = token.or_else(|| saved_github_token(&paths));
    let provider = github_provider(token.as_deref())?;
    let comparison = provider
        .compare_refs(
            &workspace,
            &GitRef::Tag(from.clone()),
            &GitRef::Tag(to.clone()),
        )
        .await
        .map_err(provider_command_error)?;
    let from_detail = skill_library_sync::read_github_skill_detail(
        &workspace,
        &skill_path,
        Some(&from),
        token.as_deref(),
    )
    .await?;
    let to_detail = skill_library_sync::read_github_skill_detail(
        &workspace,
        &skill_path,
        Some(&to),
        token.as_deref(),
    )
    .await?;
    let files = comparison
        .files
        .into_iter()
        .filter(|file| file.filename.starts_with(&skill_path))
        .collect();
    Ok(SkillComparison {
        workspace,
        skill_path,
        from,
        to,
        files,
        semantic: skill_library_manifest::semantic_diff(
            &from_detail.asset.manifest,
            &to_detail.asset.manifest,
        ),
    })
}

#[tauri::command]
fn parse_skill(path: String) -> CommandResult<skill_library_manifest::ManifestParseResult> {
    Ok(skill_library_manifest::parse_skill_dir(path)?)
}

#[tauri::command]
fn read_subscriptions() -> CommandResult<SubscriptionsFile> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    Ok(skill_library_sync::read_subscriptions(
        &paths.subscriptions,
    )?)
}

#[tauri::command]
fn subscribe_workspace_skill(
    workspace: String,
    asset_id: String,
    version: Option<String>,
    targets: Vec<String>,
) -> CommandResult<SubscriptionsFile> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let subscription = Subscription {
        workspace,
        asset_id,
        channel: "stable".to_owned(),
        version,
        update: skill_library_core::UpdatePolicy::Manual,
        targets: selection_from_targets(targets),
        subscribed_at: None,
    };
    Ok(skill_library_sync::subscribe(
        &paths.subscriptions,
        subscription,
    )?)
}

/// Download + install all subscribed skills (GitHub archive → extract → install
/// → pin lockfile). This is the real remote-install path that `install_skill`
/// (local dirs only) cannot perform. Used by consumer one-click install (after
/// subscribe) and by "auto update". `allow_risky` must be true to install
/// medium-or-higher risk skills without erroring.
#[tauri::command]
async fn sync_now(allow_risky: Option<bool>) -> CommandResult<SyncReport> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths);
    let report = skill_library_sync::sync_subscriptions(
        &paths,
        SyncOptions {
            token,
            target_roots: Vec::new(),
            source_override: None,
            allow_risky: allow_risky.unwrap_or(false),
        },
    )
    .await?;
    Ok(report)
}

#[tauri::command]
fn install_skill(
    source: String,
    targets: Vec<String>,
    confirmed_risk: Option<bool>,
    project_targets: Option<Vec<ProjectInstallTarget>>,
) -> CommandResult<InstallReport> {
    let source_dir = PathBuf::from(source);
    let parse_result = skill_library_manifest::parse_skill_dir(&source_dir)?;
    let manifest = parse_result.manifest.ok_or_else(|| {
        CommandError::coded(
            "invalid_skill_source",
            format!("invalid skill source: {:?}", parse_result.errors),
        )
    })?;
    let risk_level = effective_risk(&manifest);
    if risk_level.requires_confirmation() && confirmed_risk != Some(true) {
        return Err(CommandError::coded(
            "risk_confirmation_required",
            format!(
                "risk confirmation required for {}: {} risk permissions [{}]",
                manifest.id,
                risk_level,
                manifest.permissions.join(", ")
            ),
        ));
    }
    let mut report = InstallReport {
        manifest,
        installed: Vec::new(),
        skipped: Vec::new(),
    };

    if !targets.is_empty() {
        merge_install_report(
            &mut report,
            skill_library_installer::install(InstallOptions {
                source_dir: source_dir.clone(),
                targets,
                target_roots: Vec::<TargetRoot>::new(),
            })?,
        );
    }

    for target in project_targets.unwrap_or_default() {
        let project_root = normalize_project_root(&target.project_root)?;
        let root = project_runtime_root(&project_root, &target.runtime).ok_or_else(|| {
            CommandError::coded(
                "unsupported_runtime",
                format!("runtime '{}' is not supported", target.runtime),
            )
        })?;
        merge_install_report(
            &mut report,
            skill_library_installer::install(InstallOptions {
                source_dir: source_dir.clone(),
                targets: vec![target.runtime.clone()],
                target_roots: vec![TargetRoot {
                    target: target.runtime,
                    root,
                }],
            })?,
        );
    }

    Ok(report)
}

fn merge_install_report(report: &mut InstallReport, next: InstallReport) {
    report.installed.extend(next.installed);
    report.skipped.extend(next.skipped);
}

#[tauri::command]
fn remove_skill(skill_id: String, targets: Vec<String>) -> CommandResult<Vec<PathBuf>> {
    Ok(skill_library_installer::remove(
        &skill_id,
        &targets,
        Vec::<TargetRoot>::new(),
    )?)
}

// ---------------------------------------------------------------------------
// SQLite-backed skill management
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedSkill {
    id: String,
    name: String,
    description: String,
    version: String,
    source_workspace: String,
    source_path: String,
    source_branch: String,
    local_path: String,
    link_mode: String,
    baseline_hash: String,
    is_modified: bool,
    installed_at: String,
    updated_at: String,
    /// 'downloading' | 'installed' | 'error'
    install_status: String,
    /// 0..=100, or -1 when the stream length is unknown (indeterminate bar).
    download_progress: i64,
    download_error: String,
    /// '' (never reviewed) | 'safe' | 'caution' | 'danger'
    review_verdict: String,
    review_summary: String,
    review_findings: Vec<ai_review::ReviewFinding>,
    /// RFC3339 timestamp of the last review, or '' if never reviewed.
    reviewed_at: String,
    /// True when a review exists but the skill's content changed since — the
    /// cached verdict is shown as stale and the user is nudged to re-review.
    review_stale: bool,
    targets: Vec<ManagedSkillTarget>,
    project_deployments: Vec<ManagedSkillProjectDeployment>,
}

/// Build the review fields of a `ManagedSkill` from a stored row. `current_hash`
/// is the skill's present content hash, used to flag a stale (outdated) verdict.
fn review_fields_from_row(
    row: &db::SkillRow,
    current_hash: &str,
) -> (String, String, Vec<ai_review::ReviewFinding>, String, bool) {
    let findings: Vec<ai_review::ReviewFinding> = if row.review_findings_json.is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&row.review_findings_json).unwrap_or_default()
    };
    // Stale only makes sense once a review exists and we have a hash to compare.
    let stale = !row.review_verdict.is_empty()
        && !row.reviewed_hash.is_empty()
        && !current_hash.is_empty()
        && row.reviewed_hash != current_hash;
    (
        row.review_verdict.clone(),
        row.review_summary.clone(),
        findings,
        row.reviewed_at.clone(),
        stale,
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedSkillTarget {
    runtime: String,
    enabled: bool,
    target_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectInstallTarget {
    runtime: String,
    project_root: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagedSkillProjectDeployment {
    id: i64,
    runtime: String,
    project_root: String,
    target_path: String,
    enabled: bool,
    status: String,
    installed_hash: String,
    last_seen_hash: String,
    installed_at: String,
    updated_at: String,
    last_checked_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PathOpener {
    id: String,
    label: String,
    app_name: Option<String>,
    icon_url: Option<String>,
    icon_urls: Option<PathOpenerIconUrls>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PathOpenerIconUrls {
    small: String,
    #[serde(rename = "default")]
    default_size: String,
    large: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnmanagedSkillInfo {
    id: String,
    name: String,
    path: String,
    found_in: Vec<String>,
    locations: Vec<UnmanagedSkillLocationInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnmanagedSkillLocationInfo {
    runtime: String,
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupportedRuntime {
    id: String,
    label: String,
    global_path: String,
    exists: bool,
}

/// List all supported runtimes and whether their directories exist.
#[tauri::command]
fn db_list_runtimes() -> CommandResult<Vec<SupportedRuntime>> {
    let home = dirs::home_dir().ok_or_else(|| {
        CommandError::coded("home_dir_unavailable", "cannot resolve home directory")
    })?;
    Ok(db::SUPPORTED_RUNTIMES
        .iter()
        .map(|r| {
            let path = home.join(r.global_path);
            SupportedRuntime {
                id: r.id.to_owned(),
                label: r.label.to_owned(),
                global_path: r.global_path.to_owned(),
                exists: path.is_dir(),
            }
        })
        .collect())
}

/// List all managed skills from SQLite with their target states.
#[tauri::command]
fn db_list_skills(app: tauri::AppHandle) -> CommandResult<Vec<ManagedSkill>> {
    let database = app.state::<Mutex<db::Database>>();
    let db = database.lock().unwrap();
    let skills = db
        .list_skills()
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    let all_targets = db
        .get_all_targets()
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    let all_project_deployments = db
        .list_project_deployments()
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;

    Ok(skills
        .into_iter()
        .map(|s| {
            let targets: Vec<ManagedSkillTarget> = all_targets
                .iter()
                .filter(|t| t.skill_id == s.id)
                .map(|t| ManagedSkillTarget {
                    runtime: t.runtime.clone(),
                    enabled: t.enabled,
                    target_path: t.target_path.clone(),
                })
                .collect();
            let project_deployments: Vec<ManagedSkillProjectDeployment> = all_project_deployments
                .iter()
                .filter(|deployment| deployment.skill_id == s.id)
                .map(|deployment| ManagedSkillProjectDeployment {
                    id: deployment.id,
                    runtime: deployment.runtime.clone(),
                    project_root: deployment.project_root.clone(),
                    target_path: deployment.target_path.clone(),
                    enabled: deployment.enabled,
                    status: deployment.status.clone(),
                    installed_hash: deployment.installed_hash.clone(),
                    last_seen_hash: deployment.last_seen_hash.clone(),
                    installed_at: deployment.installed_at.clone(),
                    updated_at: deployment.updated_at.clone(),
                    last_checked_at: deployment.last_checked_at.clone(),
                })
                .collect();
            // Use baseline_hash (updated on download/sync) as the staleness
            // anchor: a re-download or remote update changes it, marking any
            // earlier review stale, without an expensive per-list dir hash.
            let (review_verdict, review_summary, review_findings, reviewed_at, review_stale) =
                review_fields_from_row(&s, &s.baseline_hash);
            ManagedSkill {
                id: s.id,
                name: s.name,
                description: s.description,
                version: s.version,
                source_workspace: s.source_workspace,
                source_path: s.source_path,
                source_branch: s.source_branch,
                local_path: s.local_path,
                link_mode: s.link_mode,
                baseline_hash: s.baseline_hash,
                is_modified: s.is_modified,
                installed_at: s.installed_at,
                updated_at: s.updated_at,
                install_status: s.install_status,
                download_progress: s.download_progress,
                download_error: s.download_error,
                review_verdict,
                review_summary,
                review_findings,
                reviewed_at,
                review_stale,
                targets,
                project_deployments,
            }
        })
        .collect())
}

/// Enable a skill for a specific runtime (create symlink/copy).
#[tauri::command]
fn db_enable_skill(app: tauri::AppHandle, skill_id: String, runtime: String) -> CommandResult<()> {
    let home = dirs::home_dir().ok_or_else(|| {
        CommandError::coded("home_dir_unavailable", "cannot resolve home directory")
    })?;
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();

    let skill = db_guard
        .get_skill(&skill_id)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?
        .ok_or_else(|| {
            CommandError::coded(
                "skill_not_found",
                format!("skill '{}' not in registry", skill_id),
            )
        })?;

    let target_dir = db::resolve_runtime_global_path(&home, &runtime).ok_or_else(|| {
        CommandError::coded(
            "unsupported_runtime",
            format!("runtime '{}' is not supported", runtime),
        )
    })?;
    let target_path = target_dir.join(&skill_id);
    let source_path = PathBuf::from(&skill.local_path);

    db::link_skill(&source_path, &target_path, &skill.link_mode)
        .map_err(|e| CommandError::coded("link_error", e.to_string()))?;

    db_guard
        .set_target_enabled(&skill_id, &runtime, true, &target_path.to_string_lossy())
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;

    Ok(())
}

/// Disable a skill for a specific runtime (remove symlink/copy).
#[tauri::command]
fn db_disable_skill(app: tauri::AppHandle, skill_id: String, runtime: String) -> CommandResult<()> {
    let home = dirs::home_dir().ok_or_else(|| {
        CommandError::coded("home_dir_unavailable", "cannot resolve home directory")
    })?;
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();

    let target_dir = db::resolve_runtime_global_path(&home, &runtime).ok_or_else(|| {
        CommandError::coded(
            "unsupported_runtime",
            format!("runtime '{}' is not supported", runtime),
        )
    })?;
    let target_path = target_dir.join(&skill_id);

    db::unlink_skill(&target_path)
        .map_err(|e| CommandError::coded("unlink_error", e.to_string()))?;

    db_guard
        .set_target_enabled(&skill_id, &runtime, false, &target_path.to_string_lossy())
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;

    Ok(())
}

fn normalize_project_root(project_root: &str) -> CommandResult<PathBuf> {
    let raw = PathBuf::from(project_root.trim());
    if !raw.is_dir() {
        return Err(CommandError::coded(
            "invalid_project_root",
            format!("'{}' is not a project directory", raw.display()),
        ));
    }
    raw.canonicalize()
        .map_err(|err| CommandError::coded("invalid_project_root", err.to_string()))
}

fn project_runtime_root(project_root: &Path, runtime: &str) -> Option<PathBuf> {
    db::SUPPORTED_RUNTIMES
        .iter()
        .find(|r| r.id == runtime)
        .map(|r| project_root.join(r.global_path))
}

fn project_target_path(
    project_root: &Path,
    runtime: &str,
    skill_id: &str,
) -> CommandResult<PathBuf> {
    let root = project_runtime_root(project_root, runtime).ok_or_else(|| {
        CommandError::coded(
            "unsupported_runtime",
            format!("runtime '{}' is not supported", runtime),
        )
    })?;
    Ok(root.join(skill_id))
}

fn link_project_deployments(
    db_guard: &db::Database,
    skill_id: &str,
    source_path: &Path,
    project_targets: &[ProjectInstallTarget],
    link_mode: &str,
    installed_hash: &str,
) -> CommandResult<()> {
    for target in project_targets {
        let project_root = normalize_project_root(&target.project_root)?;
        let target_path = project_target_path(&project_root, &target.runtime, skill_id)?;
        db::link_skill(source_path, &target_path, link_mode)
            .map_err(|e| CommandError::coded("link_error", e.to_string()))?;
        db_guard
            .upsert_project_deployment(
                skill_id,
                &target.runtime,
                &project_root.to_string_lossy(),
                &target_path.to_string_lossy(),
                installed_hash,
            )
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
fn db_check_project_deployments(app: tauri::AppHandle) -> CommandResult<usize> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    let rows = db_guard
        .list_project_deployments()
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    let mut changed = 0;
    for row in rows {
        if !row.enabled {
            let target_path = PathBuf::from(&row.target_path);
            if target_path.exists() || target_path.is_symlink() {
                db::unlink_skill(&target_path)
                    .map_err(|e| CommandError::coded("unlink_error", e.to_string()))?;
                changed += 1;
            }
            if row.status != "paused" {
                changed += 1;
            }
            db_guard
                .set_project_deployment_status(row.id, "paused", &row.last_seen_hash)
                .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
            continue;
        }
        let exists = PathBuf::from(&row.target_path).join("SKILL.md").is_file();
        let next_status = if exists { "active" } else { "missing" };
        if row.status != next_status {
            changed += 1;
        }
        db_guard
            .set_project_deployment_status(row.id, next_status, &row.last_seen_hash)
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    }
    Ok(changed)
}

#[tauri::command]
fn db_add_project_deployments(
    app: tauri::AppHandle,
    skill_id: String,
    project_targets: Vec<ProjectInstallTarget>,
) -> CommandResult<()> {
    if project_targets.is_empty() {
        return Err(CommandError::coded(
            "missing_project_target",
            "at least one project target is required",
        ));
    }

    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    let skill = db_guard
        .get_skill(&skill_id)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?
        .ok_or_else(|| {
            CommandError::coded("skill_not_found", format!("skill '{}' not found", skill_id))
        })?;
    let source_path = PathBuf::from(&skill.local_path);
    if !source_path.join("SKILL.md").is_file() {
        return Err(CommandError::coded(
            "skill_source_missing",
            format!("'{}' is missing SKILL.md", source_path.display()),
        ));
    }

    link_project_deployments(
        &db_guard,
        &skill_id,
        &source_path,
        &project_targets,
        &skill.link_mode,
        &skill.baseline_hash,
    )
}

#[tauri::command]
fn db_set_project_deployment_enabled(
    app: tauri::AppHandle,
    deployment_id: i64,
    enabled: bool,
) -> CommandResult<()> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    let deployment = db_guard
        .get_project_deployment(deployment_id)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?
        .ok_or_else(|| {
            CommandError::coded(
                "project_deployment_not_found",
                format!("project deployment '{}' not found", deployment_id),
            )
        })?;

    if enabled {
        let skill = db_guard
            .get_skill(&deployment.skill_id)
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?
            .ok_or_else(|| {
                CommandError::coded(
                    "skill_not_found",
                    format!("skill '{}' not found", deployment.skill_id),
                )
            })?;
        let source_path = PathBuf::from(&skill.local_path);
        if !source_path.join("SKILL.md").is_file() {
            return Err(CommandError::coded(
                "skill_source_missing",
                format!("'{}' is missing SKILL.md", source_path.display()),
            ));
        }
        let target_path = PathBuf::from(&deployment.target_path);
        if !target_path.join("SKILL.md").is_file() {
            if target_path.is_symlink() {
                db::unlink_skill(&target_path)
                    .map_err(|e| CommandError::coded("unlink_error", e.to_string()))?;
            } else if target_path.exists() {
                return Err(CommandError::coded(
                    "project_target_conflict",
                    format!("'{}' exists but is not a skill", target_path.display()),
                ));
            }
            db::link_skill(&source_path, &target_path, &skill.link_mode)
                .map_err(|e| CommandError::coded("link_error", e.to_string()))?;
        }
        db_guard
            .set_project_deployment_status(deployment_id, "active", &skill.baseline_hash)
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    } else {
        let target_path = PathBuf::from(&deployment.target_path);
        if target_path.exists() || target_path.is_symlink() {
            db::unlink_skill(&target_path)
                .map_err(|e| CommandError::coded("unlink_error", e.to_string()))?;
        }
        db_guard
            .set_project_deployment_status(deployment_id, "paused", &deployment.last_seen_hash)
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    }

    db_guard
        .set_project_deployment_enabled(deployment_id, enabled)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    Ok(())
}

#[tauri::command]
fn db_delete_project_deployment(app: tauri::AppHandle, deployment_id: i64) -> CommandResult<()> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    let deployment = db_guard
        .get_project_deployment(deployment_id)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?
        .ok_or_else(|| {
            CommandError::coded(
                "project_deployment_not_found",
                format!("project deployment '{}' not found", deployment_id),
            )
        })?;
    let target_path = PathBuf::from(&deployment.target_path);
    if target_path.exists() || target_path.is_symlink() {
        db::unlink_skill(&target_path)
            .map_err(|e| CommandError::coded("unlink_error", e.to_string()))?;
    }
    db_guard
        .delete_project_deployment(deployment_id)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    Ok(())
}

/// Scan all IDE directories for skills not managed by us.
#[tauri::command]
fn db_scan_unmanaged(app: tauri::AppHandle) -> CommandResult<Vec<UnmanagedSkillInfo>> {
    let home = dirs::home_dir().ok_or_else(|| {
        CommandError::coded("home_dir_unavailable", "cannot resolve home directory")
    })?;
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();

    let unmanaged = db::scan_unmanaged_skills(&home, &db_guard);
    Ok(unmanaged
        .into_iter()
        .map(|s| UnmanagedSkillInfo {
            id: s.id,
            name: s.name,
            path: s.path.to_string_lossy().to_string(),
            found_in: s.found_in,
            locations: s
                .locations
                .into_iter()
                .map(|location| UnmanagedSkillLocationInfo {
                    runtime: location.runtime,
                    path: location.path.to_string_lossy().to_string(),
                })
                .collect(),
        })
        .collect())
}

/// Import an unmanaged skill into our data directory and register it.
#[tauri::command]
fn db_import_skill(
    app: tauri::AppHandle,
    _skill_id: String,
    source_path: String,
    link_mode: Option<String>,
    project_targets: Option<Vec<ProjectInstallTarget>>,
) -> CommandResult<ManagedSkill> {
    let paths = AppPaths::resolve()?;
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();

    let link_mode = link_mode.unwrap_or_else(|| "symlink".to_owned());
    let source = PathBuf::from(&source_path);
    if !source.is_dir() {
        return Err(CommandError::coded(
            "invalid_skill_source",
            format!("'{}' is not a directory", source.display()),
        ));
    }

    if !source.join("SKILL.md").is_file() {
        return Err(CommandError::coded(
            "missing_skill_md",
            "selected directory must contain SKILL.md",
        ));
    }

    let source_parse = skill_library_manifest::parse_skill_dir(&source)
        .map_err(|err| CommandError::coded("invalid_skill_source", err.to_string()))?;
    let source_manifest = source_parse.manifest.ok_or_else(|| {
        CommandError::coded(
            "invalid_skill_source",
            format!("invalid SKILL.md metadata: {:?}", source_parse.errors),
        )
    })?;
    let resolved_skill_id = resolve_import_skill_id(&source_manifest.name)?;
    let dest = paths.home.join("skills").join(&resolved_skill_id);

    // Copy skill to our data directory
    if !dest.exists() {
        db::copy_dir_recursive(&source, &dest)
            .map_err(|e| CommandError::coded("copy_error", e.to_string()))?;
    }

    // Parse manifest for metadata
    let manifest = skill_library_manifest::parse_skill_dir(&dest)
        .ok()
        .and_then(|p| p.manifest);

    let name = manifest
        .as_ref()
        .map(|m| m.name.clone())
        .unwrap_or_else(|| db::humanize_name(&resolved_skill_id));
    let description = manifest
        .as_ref()
        .map(|m| m.description.clone())
        .unwrap_or_default();
    let version = manifest
        .as_ref()
        .map(|m| m.version.clone())
        .unwrap_or_else(|| "0.1.0".to_owned());

    // Compute content hash for change detection
    let baseline_hash = db::compute_dir_hash(&dest);

    db_guard
        .insert_skill(
            &resolved_skill_id,
            &name,
            &description,
            &version,
            "local",
            &source_path,
            "",
            &dest.to_string_lossy(),
            &link_mode,
            &baseline_hash,
        )
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;

    let targets = db_guard
        .get_targets_for_skill(&resolved_skill_id)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    let project_targets = project_targets.unwrap_or_default();
    if !project_targets.is_empty() {
        link_project_deployments(
            &db_guard,
            &resolved_skill_id,
            &dest,
            &project_targets,
            &link_mode,
            &baseline_hash,
        )?;
    }
    let project_deployments = db_guard
        .list_project_deployments()
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;

    Ok(ManagedSkill {
        id: resolved_skill_id.clone(),
        name,
        description,
        version,
        source_workspace: "local".to_owned(),
        source_path: source_path,
        source_branch: String::new(),
        local_path: dest.to_string_lossy().to_string(),
        link_mode,
        baseline_hash,
        is_modified: false,
        installed_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        install_status: "installed".to_owned(),
        download_progress: 100,
        download_error: String::new(),
        review_verdict: String::new(),
        review_summary: String::new(),
        review_findings: Vec::new(),
        reviewed_at: String::new(),
        review_stale: false,
        targets: targets
            .into_iter()
            .map(|t| ManagedSkillTarget {
                runtime: t.runtime,
                enabled: t.enabled,
                target_path: t.target_path,
            })
            .collect(),
        project_deployments: project_deployments
            .into_iter()
            .filter(|deployment| deployment.skill_id == resolved_skill_id)
            .map(|deployment| ManagedSkillProjectDeployment {
                id: deployment.id,
                runtime: deployment.runtime,
                project_root: deployment.project_root,
                target_path: deployment.target_path,
                enabled: deployment.enabled,
                status: deployment.status,
                installed_hash: deployment.installed_hash,
                last_seen_hash: deployment.last_seen_hash,
                installed_at: deployment.installed_at,
                updated_at: deployment.updated_at,
                last_checked_at: deployment.last_checked_at,
            })
            .collect(),
    })
}

fn resolve_import_skill_id(skill_name: &str) -> CommandResult<String> {
    let id = sanitize_skill_id(skill_name);
    if id.is_empty() {
        return Err(CommandError::coded(
            "invalid_skill_id",
            "skill id could not be inferred from SKILL.md name",
        ));
    }
    Ok(id)
}

fn sanitize_skill_id(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['-', '.', '_'])
        .to_owned()
}

/// Progress event payload for an in-flight async skill download. Emitted on the
/// `skill-download-progress` channel so My Skills can show a live bar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillDownloadProgress {
    skill_id: String,
    /// "downloading" | "installed" | "error"
    status: String,
    /// 0..=100, or -1 when the stream length is unknown (indeterminate bar).
    progress: i64,
    error: Option<String>,
}

const SKILL_DOWNLOAD_EVENT: &str = "skill-download-progress";

fn emit_download_progress(app: &tauri::AppHandle, payload: SkillDownloadProgress) {
    let _ = app.emit(SKILL_DOWNLOAD_EVENT, payload);
}

/// Mark a download as failed in SQLite and notify the UI. Keeping the row (in
/// the 'error' state) is deliberate: the user sees the failure and can retry,
/// rather than the entry silently vanishing.
fn mark_download_failed(app: &tauri::AppHandle, asset_id: &str, error: &str) {
    if let Some(database) = app.try_state::<Mutex<db::Database>>() {
        if let Ok(db_guard) = database.lock() {
            let _ = db_guard.fail_download(asset_id, error);
        }
    }
    emit_download_progress(
        app,
        SkillDownloadProgress {
            skill_id: asset_id.to_owned(),
            status: "error".to_owned(),
            progress: 0,
            error: Some(error.to_owned()),
        },
    );
}

/// Kick off an asynchronous download + install of a remote skill.
///
/// Unlike `sync_now` (which blocks until the whole tarball is fetched), this
/// returns immediately after recording a 'downloading' row in SQLite, then does
/// the network fetch + extraction + linking on a background task, emitting
/// `skill-download-progress` events with a real byte-based percentage.
///
/// Duplicate protection: if the skill is already 'downloading' or 'installed',
/// the command errors with a coded reason so the UI can show a notice instead of
/// starting a redundant download. An 'error' row (a prior failed attempt) is
/// allowed through so the user can retry.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn download_skill_async(
    app: tauri::AppHandle,
    workspace: String,
    asset_id: String,
    skill_path: Option<String>,
    version: Option<String>,
    name: Option<String>,
    description: Option<String>,
    targets: Vec<String>,
    link_mode: Option<String>,
    project_targets: Option<Vec<ProjectInstallTarget>>,
) -> CommandResult<()> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace_ref = parse_workspace(&workspace)?;

    let link_mode = link_mode.unwrap_or_else(|| "symlink".to_owned());
    let dest = paths.home.join("skills").join(&asset_id);
    let display_name = name.unwrap_or_else(|| asset_id.clone());
    let description = description.unwrap_or_default();
    let project_targets = project_targets.unwrap_or_default();
    let version = version
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
        .unwrap_or_default();

    // Duplicate protection + record the 'downloading' row up front.
    {
        let database = app.state::<Mutex<db::Database>>();
        let db_guard = database.lock().unwrap();
        if let Some(existing) = db_guard
            .get_skill(&asset_id)
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?
        {
            match existing.install_status.as_str() {
                "downloading" => {
                    return Err(CommandError::coded(
                        "already_downloading",
                        format!("'{asset_id}' is already downloading"),
                    ))
                }
                "installed" => {
                    if targets.is_empty() && project_targets.is_empty() {
                        return Err(CommandError::coded(
                            "already_installed",
                            format!("'{asset_id}' is already installed"),
                        ));
                    }
                    let source_path = PathBuf::from(&existing.local_path);
                    for runtime in &targets {
                        let Some(home) = dirs::home_dir() else {
                            continue;
                        };
                        let Some(target_dir) = db::resolve_runtime_global_path(&home, runtime)
                        else {
                            continue;
                        };
                        let target_path = target_dir.join(&asset_id);
                        db::link_skill(&source_path, &target_path, &existing.link_mode)
                            .map_err(|e| CommandError::coded("link_error", e.to_string()))?;
                        db_guard
                            .set_target_enabled(
                                &asset_id,
                                runtime,
                                true,
                                &target_path.to_string_lossy(),
                            )
                            .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
                    }
                    link_project_deployments(
                        &db_guard,
                        &asset_id,
                        &source_path,
                        &project_targets,
                        &existing.link_mode,
                        &existing.baseline_hash,
                    )?;
                    emit_download_progress(
                        &app,
                        SkillDownloadProgress {
                            skill_id: asset_id,
                            status: "installed".to_owned(),
                            progress: 100,
                            error: None,
                        },
                    );
                    return Ok(());
                }
                "error" => {}
                _ if !targets.is_empty() || !project_targets.is_empty() => {
                    return Err(CommandError::coded(
                        "already_installed",
                        format!("'{asset_id}' is already installed"),
                    ))
                }
                // "error" (a prior failed attempt) → allow retry.
                _ => {}
            }
        }
        db_guard
            .begin_download(
                &asset_id,
                &display_name,
                &description,
                &version,
                &workspace,
                skill_path.as_deref().unwrap_or(""),
                "",
                &dest.to_string_lossy(),
                &link_mode,
            )
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    }

    emit_download_progress(
        &app,
        SkillDownloadProgress {
            skill_id: asset_id.clone(),
            status: "downloading".to_owned(),
            progress: 0,
            error: None,
        },
    );

    let token = saved_github_token(&paths);
    tauri::async_runtime::spawn(run_skill_download(
        app,
        paths,
        workspace_ref,
        asset_id,
        skill_path,
        version,
        targets,
        project_targets,
        link_mode,
        dest,
        token,
    ));

    Ok(())
}

/// Background worker for [`download_skill_async`]: fetch the tarball with
/// progress, copy the located skill into the data dir, link it into the chosen
/// tool folders, and reconcile the SQLite row to 'installed' (or 'error').
#[allow(clippy::too_many_arguments)]
async fn run_skill_download(
    app: tauri::AppHandle,
    paths: AppPaths,
    workspace: WorkspaceRef,
    asset_id: String,
    skill_path: Option<String>,
    version: String,
    targets: Vec<String>,
    project_targets: Vec<ProjectInstallTarget>,
    link_mode: String,
    dest: PathBuf,
    token: Option<String>,
) {
    // Throttle progress writes/emits to once per whole-percent change.
    let progress_app = app.clone();
    let progress_id = asset_id.clone();
    let mut last_percent: i64 = -2;
    let on_progress = move |downloaded: u64, total: Option<u64>| {
        let percent = match total {
            Some(total) if total > 0 => ((downloaded.saturating_mul(100)) / total) as i64,
            _ => -1,
        };
        if percent == last_percent {
            return;
        }
        last_percent = percent;
        if let Some(database) = progress_app.try_state::<Mutex<db::Database>>() {
            if let Ok(db_guard) = database.lock() {
                let _ = db_guard.set_download_progress(&progress_id, percent);
            }
        }
        emit_download_progress(
            &progress_app,
            SkillDownloadProgress {
                skill_id: progress_id.clone(),
                status: "downloading".to_owned(),
                progress: percent,
                error: None,
            },
        );
    };

    let prepared = match skill_library_sync::download_skill_for_install(
        &paths,
        &workspace,
        &asset_id,
        skill_path.as_deref(),
        &version,
        token.as_deref(),
        on_progress,
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(err) => {
            mark_download_failed(&app, &asset_id, &err.to_string());
            return;
        }
    };

    // Copy the located skill into our managed data dir (fresh each time so a
    // retry overwrites any partial leftovers).
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    if let Err(err) = db::copy_dir_recursive(&prepared.source_dir, &dest) {
        mark_download_failed(&app, &asset_id, &err.to_string());
        return;
    }
    let baseline_hash = db::compute_dir_hash(&dest);

    {
        let prepared_source_path = prepared.source_path.to_string_lossy().to_string();
        let database = app.state::<Mutex<db::Database>>();
        let lock = database.lock();
        if let Ok(db_guard) = lock {
            if let Err(err) = db_guard.finish_download(
                &asset_id,
                &prepared.manifest.name,
                &prepared.manifest.description,
                &prepared.manifest.version,
                &prepared_source_path,
                &prepared.ref_name,
                &dest.to_string_lossy(),
                &baseline_hash,
            ) {
                drop(db_guard);
                mark_download_failed(&app, &asset_id, &err.to_string());
                return;
            }
        }
    }

    // Link into each chosen tool folder (empty targets = download-only).
    if let Some(home) = dirs::home_dir() {
        for runtime in &targets {
            let Some(target_dir) = db::resolve_runtime_global_path(&home, runtime) else {
                continue;
            };
            let target_path = target_dir.join(&asset_id);
            match db::link_skill(&dest, &target_path, &link_mode) {
                Ok(()) => {
                    let database = app.state::<Mutex<db::Database>>();
                    let lock = database.lock();
                    if let Ok(db_guard) = lock {
                        let _ = db_guard.set_target_enabled(
                            &asset_id,
                            runtime,
                            true,
                            &target_path.to_string_lossy(),
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(skill = %asset_id, runtime = %runtime, error = %err, "failed to link skill into tool folder");
                }
            }
        }
    }

    {
        let database = app.state::<Mutex<db::Database>>();
        let lock = database.lock();
        if let Ok(db_guard) = lock {
            if let Err(err) = link_project_deployments(
                &db_guard,
                &asset_id,
                &dest,
                &project_targets,
                &link_mode,
                &baseline_hash,
            ) {
                tracing::warn!(skill = %asset_id, error = %err, "failed to link skill into project folder");
            }
        }
    }

    emit_download_progress(
        &app,
        SkillDownloadProgress {
            skill_id: asset_id,
            status: "installed".to_owned(),
            progress: 100,
            error: None,
        },
    );
}

/// Open the Skill Library data directory in the system file manager.
#[tauri::command]
fn open_data_dir(app: tauri::AppHandle) -> CommandResult<()> {
    let paths = AppPaths::resolve()?;
    app.opener()
        .open_path(paths.home.to_string_lossy().to_string(), None::<&str>)
        .map_err(|err| CommandError::coded("open_dir_failed", err.to_string()))?;
    Ok(())
}

#[tauri::command]
fn open_local_path(
    app: tauri::AppHandle,
    path: String,
    opener: Option<String>,
) -> CommandResult<()> {
    let path = PathBuf::from(path.trim());
    if !path.exists() {
        return Err(CommandError::coded(
            "path_not_found",
            format!("'{}' does not exist", path.display()),
        ));
    }
    let opener = opener
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    match opener.as_deref() {
        None | Some("default") => open_with_system_default(&app, &path)?,
        Some(token) => {
            if let Some(candidate) = find_path_opener_candidate(token) {
                open_with_candidate(&app, &path, candidate)?;
            } else {
                app.opener()
                    .open_path(path.to_string_lossy().to_string(), Some(token))
                    .map_err(|err| CommandError::coded("open_path_failed", err.to_string()))?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn list_path_openers() -> CommandResult<Vec<PathOpener>> {
    Ok(available_path_openers())
}

fn available_path_openers() -> Vec<PathOpener> {
    let mut openers: Vec<PathOpener> = app_icons::candidates()
        .iter()
        .filter(|candidate| path_opener_available(candidate))
        .map(path_opener)
        .collect();
    if openers.is_empty() {
        openers.push(PathOpener {
            id: "default".to_owned(),
            label: "Default".to_owned(),
            app_name: None,
            icon_url: None,
            icon_urls: None,
        });
    }
    openers
}

fn path_opener_available(candidate: &app_icons::PathOpenerCandidate) -> bool {
    candidate
        .app_name
        .and_then(app_icons::find_app_bundle)
        .is_some()
        || find_candidate_cli(candidate).is_some()
}

fn path_opener(candidate: &app_icons::PathOpenerCandidate) -> PathOpener {
    let icon_urls = PathOpenerIconUrls {
        small: app_icons::icon_url(candidate.id, app_icons::IconSize::Small),
        default_size: app_icons::icon_url(candidate.id, app_icons::IconSize::Default),
        large: app_icons::icon_url(candidate.id, app_icons::IconSize::Large),
    };
    PathOpener {
        id: candidate.id.to_owned(),
        label: candidate.label.to_owned(),
        app_name: candidate.app_name.map(str::to_owned),
        icon_url: Some(icon_urls.default_size.clone()),
        icon_urls: Some(icon_urls),
    }
}

fn find_path_opener_candidate(token: &str) -> Option<&'static app_icons::PathOpenerCandidate> {
    app_icons::find_candidate(token)
}

fn open_with_system_default(app: &tauri::AppHandle, path: &Path) -> CommandResult<()> {
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|err| CommandError::coded("open_path_failed", err.to_string()))?;
    Ok(())
}

fn open_with_candidate(
    app: &tauri::AppHandle,
    path: &Path,
    candidate: &app_icons::PathOpenerCandidate,
) -> CommandResult<()> {
    if candidate.id == "finder" {
        spawn_path_command("/usr/bin/open", ["-R"], path)?;
        return Ok(());
    }

    if let Some(cli) = find_candidate_cli(candidate) {
        Command::new(&cli).arg(path).spawn().map_err(|err| {
            CommandError::coded(
                "open_path_failed",
                format!("failed to launch {}: {err}", cli.display()),
            )
        })?;
        return Ok(());
    }

    if let Some(app_name) = candidate.app_name {
        app.opener()
            .open_path(path.to_string_lossy().to_string(), Some(app_name))
            .map_err(|err| CommandError::coded("open_path_failed", err.to_string()))?;
        return Ok(());
    }

    open_with_system_default(app, path)
}

fn spawn_path_command<const N: usize>(
    command: &str,
    args: [&str; N],
    path: &Path,
) -> CommandResult<()> {
    let mut cmd = Command::new(command);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.arg(path).spawn().map_err(|err| {
        CommandError::coded(
            "open_path_failed",
            format!("failed to launch {command}: {err}"),
        )
    })?;
    Ok(())
}

fn find_candidate_cli(candidate: &app_icons::PathOpenerCandidate) -> Option<PathBuf> {
    if let Some(app_name) = candidate.app_name {
        if let Some(bundle_path) = app_icons::find_app_bundle(app_name) {
            for relative_path in candidate.bundle_cli_paths {
                let path = bundle_path.join(relative_path);
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    for name in candidate.cli_names {
        if let Some(path) = find_executable_in_path(name) {
            return Some(path);
        }
    }
    None
}

fn find_executable_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for root in std::env::split_paths(&path_var) {
        let path = root.join(name);
        if path.exists() {
            return Some(path);
        }

        #[cfg(target_os = "windows")]
        {
            for suffix in [".exe", ".cmd"] {
                if name.ends_with(suffix) {
                    continue;
                }
                let path = root.join(format!("{name}{suffix}"));
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Get cache size breakdown by workspace (from SQLite).
#[tauri::command]
fn db_cache_stats(app: tauri::AppHandle) -> CommandResult<Vec<CacheSizeInfo>> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    let rows = db_guard
        .cache_size_by_workspace()
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|r| CacheSizeInfo {
            workspace: r.workspace,
            count: r.count,
            total_bytes: r.total_bytes,
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheSizeInfo {
    workspace: String,
    count: i64,
    total_bytes: i64,
}

/// Clear cache for a specific workspace.
#[tauri::command]
fn db_clear_cache(app: tauri::AppHandle, workspace: Option<String>) -> CommandResult<()> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    match workspace {
        Some(ws) => db_guard
            .clear_cache_for_workspace(&ws)
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?,
        None => db_guard
            .clear_all_cache()
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?,
    }
    Ok(())
}

/// Get a cache entry by key (returns base64-encoded data or null).
#[tauri::command]
fn db_cache_get(app: tauri::AppHandle, key: String) -> CommandResult<Option<String>> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    let data = db_guard
        .get_cache(&key)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    Ok(data.map(|bytes| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    }))
}

/// Get cache entries by key (returns base64-encoded data or null, index-aligned).
#[tauri::command]
fn db_cache_get_many(
    app: tauri::AppHandle,
    keys: Vec<String>,
) -> CommandResult<Vec<Option<String>>> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    let entries = db_guard
        .get_cache_many(&keys)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    Ok(entries
        .into_iter()
        .map(|entry| {
            entry.map(|bytes| {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            })
        })
        .collect())
}

/// Put a cache entry (data is base64-encoded string from frontend).
#[tauri::command]
fn db_cache_put(
    app: tauri::AppHandle,
    key: String,
    workspace: String,
    data: String,
) -> CommandResult<()> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|e| CommandError::coded("decode_error", e.to_string()))?;
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    db_guard
        .put_cache(&key, &workspace, &bytes)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    Ok(())
}

/// Delete a single cache entry by key.
#[tauri::command]
fn db_cache_delete(app: tauri::AppHandle, key: String) -> CommandResult<()> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    db_guard
        .delete_cache(&key)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    Ok(())
}

/// Delete all cache entries whose key starts with the given prefix.
#[tauri::command]
fn db_cache_delete_prefix(app: tauri::AppHandle, prefix: String) -> CommandResult<usize> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    let count = db_guard
        .delete_cache_by_prefix(&prefix)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Filesystem-based remote file cache (~/.skill-library/remote/)
// ---------------------------------------------------------------------------

/// Resolve the remote cache root: ~/.skill-library/remote/
fn remote_cache_root() -> CommandResult<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| CommandError::coded("no_home", "cannot resolve home directory"))?;
    Ok(home.join(".skill-library").join("remote"))
}

/// Validate a path segment to prevent path traversal attacks.
fn validate_path_segment(segment: &str) -> CommandResult<()> {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains('\\')
        || segment.contains('\0')
        || segment.starts_with('/')
    {
        return Err(CommandError::coded(
            "invalid_path",
            format!("invalid path segment: {:?}", segment),
        ));
    }
    Ok(())
}

/// Build a safe cache file path: remote/{workspace}/{ref}/{file_path}
fn build_cache_path(workspace: &str, ref_name: &str, file_path: &str) -> CommandResult<PathBuf> {
    let root = remote_cache_root()?;
    // workspace is like "owner/repo" — validate each part
    for part in workspace.split('/') {
        validate_path_segment(part)?;
    }
    validate_path_segment(ref_name)?;
    // file_path can have nested dirs like "skills/code-reviewer/SKILL.md"
    for part in file_path.split('/') {
        validate_path_segment(part)?;
    }
    Ok(root.join(workspace).join(ref_name).join(file_path))
}

fn write_remote_cache_file_bytes(
    workspace: &str,
    ref_name: &str,
    file_path: &str,
    bytes: &[u8],
) -> CommandResult<()> {
    let target = build_cache_path(workspace, ref_name, file_path)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CommandError::coded("io_error", format!("mkdir failed: {}", e)))?;
    }
    fs::write(&target, bytes)
        .map_err(|e| CommandError::coded("io_error", format!("write failed: {}", e)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

/// Write a file to the remote cache. Binary data is passed as base64.
#[tauri::command]
fn remote_cache_put_file(
    workspace: String,
    ref_name: String,
    file_path: String,
    data: String,
    is_binary: bool,
) -> CommandResult<()> {
    let bytes = if is_binary {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|e| CommandError::coded("decode_error", e.to_string()))?;
        bytes
    } else {
        data.into_bytes()
    };
    write_remote_cache_file_bytes(&workspace, &ref_name, &file_path, &bytes)
}

/// Read a file from the remote cache. Returns content + is_binary flag.
#[tauri::command]
fn remote_cache_get_file(
    workspace: String,
    ref_name: String,
    file_path: String,
) -> CommandResult<Option<CachedFileResult>> {
    let target = build_cache_path(&workspace, &ref_name, &file_path)?;
    if !target.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&target)
        .map_err(|e| CommandError::coded("io_error", format!("read failed: {}", e)))?;
    // Try UTF-8; if fails, return as base64
    match String::from_utf8(bytes.clone()) {
        Ok(text) => Ok(Some(CachedFileResult {
            content: text,
            is_binary: false,
        })),
        Err(_) => Ok(Some(CachedFileResult {
            content: base64::engine::general_purpose::STANDARD.encode(&bytes),
            is_binary: true,
        })),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedFileResult {
    content: String,
    is_binary: bool,
}

/// Delete all cached files for a specific skill path within a workspace.
#[tauri::command]
fn remote_cache_delete_skill(workspace: String, skill_path: String) -> CommandResult<()> {
    let root = remote_cache_root()?;
    for part in workspace.split('/') {
        validate_path_segment(part)?;
    }
    for part in skill_path.split('/') {
        validate_path_segment(part)?;
    }
    let ws_dir = root.join(&workspace);
    if !ws_dir.exists() {
        return Ok(());
    }
    // Walk all ref dirs under workspace and delete the skill_path subtree
    if let Ok(entries) = fs::read_dir(&ws_dir) {
        for entry in entries.flatten() {
            let ref_dir = entry.path();
            if ref_dir.is_dir() {
                let skill_dir = ref_dir.join(&skill_path);
                if skill_dir.exists() {
                    let _ = fs::remove_dir_all(&skill_dir);
                }
            }
        }
    }
    Ok(())
}

/// Delete all cached files for a workspace.
#[tauri::command]
fn remote_cache_delete_workspace(workspace: String) -> CommandResult<()> {
    let root = remote_cache_root()?;
    for part in workspace.split('/') {
        validate_path_segment(part)?;
    }
    let ws_dir = root.join(&workspace);
    if ws_dir.exists() {
        fs::remove_dir_all(&ws_dir)
            .map_err(|e| CommandError::coded("io_error", format!("remove failed: {}", e)))?;
    }
    Ok(())
}

/// Get cache size stats for the remote file cache.
#[tauri::command]
fn remote_cache_stats() -> CommandResult<Vec<RemoteCacheStat>> {
    let root = remote_cache_root()?;
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut stats: Vec<RemoteCacheStat> = Vec::new();
    // Walk top-level: owner dirs
    if let Ok(owners) = fs::read_dir(&root) {
        for owner_entry in owners.flatten() {
            let owner_path = owner_entry.path();
            if !owner_path.is_dir() {
                continue;
            }
            let owner_name = owner_entry.file_name().to_string_lossy().to_string();
            // Walk repo dirs under owner
            if let Ok(repos) = fs::read_dir(&owner_path) {
                for repo_entry in repos.flatten() {
                    let repo_path = repo_entry.path();
                    if !repo_path.is_dir() {
                        continue;
                    }
                    let repo_name = repo_entry.file_name().to_string_lossy().to_string();
                    let workspace = format!("{}/{}", owner_name, repo_name);
                    let (total_bytes, file_count) = dir_size_recursive(&repo_path);
                    stats.push(RemoteCacheStat {
                        workspace,
                        total_bytes: total_bytes as i64,
                        file_count: file_count as i64,
                    });
                }
            }
        }
    }
    stats.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));
    Ok(stats)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteCacheStat {
    workspace: String,
    total_bytes: i64,
    file_count: i64,
}

fn dir_size_recursive(path: &Path) -> (u64, u64) {
    let mut total: u64 = 0;
    let mut count: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let (sub_size, sub_count) = dir_size_recursive(&p);
                total += sub_size;
                count += sub_count;
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
                count += 1;
            }
        }
    }
    (total, count)
}

/// Check all managed skills for local modifications (mtime pre-check + hash).
#[tauri::command]
fn db_check_modifications(app: tauri::AppHandle) -> CommandResult<Vec<String>> {
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();
    let skills = db_guard
        .list_skills()
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    let mut modified_ids = Vec::new();
    for skill in &skills {
        let is_mod = db_guard
            .check_modified(&skill.id, &skill.local_path)
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
        if is_mod {
            modified_ids.push(skill.id.clone());
        }
    }
    Ok(modified_ids)
}

/// Unmanage a skill: remove from registry, replace symlinks with real copies in IDE dirs.
#[tauri::command]
fn db_unmanage_skill(app: tauri::AppHandle, skill_id: String) -> CommandResult<()> {
    let home = dirs::home_dir().ok_or_else(|| {
        CommandError::coded("home_dir_unavailable", "cannot resolve home directory")
    })?;
    let database = app.state::<Mutex<db::Database>>();
    let db_guard = database.lock().unwrap();

    let targets = db_guard
        .get_targets_for_skill(&skill_id)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;

    let skill = db_guard
        .get_skill(&skill_id)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?
        .ok_or_else(|| {
            CommandError::coded("skill_not_found", format!("skill '{}' not found", skill_id))
        })?;

    let source_path = PathBuf::from(&skill.local_path);

    // For each enabled target: remove symlink, copy real files back
    for target in &targets {
        if !target.enabled {
            continue;
        }
        let target_path = if !target.target_path.is_empty() {
            PathBuf::from(&target.target_path)
        } else {
            match db::resolve_runtime_global_path(&home, &target.runtime) {
                Some(dir) => dir.join(&skill_id),
                None => continue,
            }
        };

        // Remove symlink
        let _ = db::unlink_skill(&target_path);
        // Copy real files back
        if source_path.is_dir() {
            let _ = db::copy_dir_recursive(&source_path, &target_path);
        }
    }

    // Remove from SQLite
    db_guard
        .unmanage_skill(&skill_id)
        .map_err(|e| CommandError::coded("db_error", e.to_string()))?;

    // Optionally remove from our data directory (keep it for safety)
    // Users can manually delete ~/.skill-library/skills/{id} if they want

    Ok(())
}

#[tauri::command]
fn list_installed_targets(
    targets: Option<Vec<String>>,
) -> CommandResult<Vec<InstalledTargetGroup>> {
    let target_list = targets.unwrap_or_else(default_runtime_targets);
    target_list
        .into_iter()
        .map(|target| {
            let skills =
                skill_library_installer::list_installed(&target, Vec::<TargetRoot>::new())?;
            Ok(InstalledTargetGroup { target, skills })
        })
        .collect()
}

#[tauri::command]
fn list_local_agent_roots() -> CommandResult<Vec<LocalAgentRoot>> {
    let home = dirs::home_dir().ok_or_else(|| {
        CommandError::coded(
            "home_dir_unavailable",
            "cannot resolve the current user's home directory",
        )
    })?;
    Ok(local_agent_root_specs(&home)
        .into_iter()
        .map(scan_local_agent_root)
        .collect())
}

#[tauri::command]
fn preview_publish(
    source: String,
    workspace: Option<String>,
    user: Option<String>,
) -> CommandResult<PublishPreview> {
    let package = skill_library_publish::package_skill(source)?;
    let policy = skill_library_publish::evaluate_publish_policy(&package)?;
    let request = match workspace {
        Some(workspace) => Some(skill_library_publish::build_publish_request(
            &package,
            &parse_workspace(&workspace)?,
            user.as_deref().unwrap_or("local"),
        )),
        None => None,
    };
    Ok(PublishPreview {
        package,
        policy,
        request,
    })
}

/// Pull a skill out of a remote workspace into a temp directory so
/// `package_skill` can re-use the existing local-disk publish pipeline.
async fn fetch_remote_skill_to_temp(
    provider: &GitHubProvider,
    paths: &AppPaths,
    source: &WorkspaceRef,
    skill_path: &str,
    git_ref: &GitRef,
    rename_to: Option<&str>,
) -> CommandResult<PathBuf> {
    let trimmed_path = skill_path.trim_matches('/').to_owned();
    if trimmed_path.is_empty() {
        return Err(CommandError::coded(
            "invalid_skill_path",
            "skill path inside the workspace cannot be empty",
        ));
    }

    let files = provider
        .list_files(source, git_ref)
        .await
        .map_err(provider_command_error)?;

    let prefix = format!("{trimmed_path}/");
    let entries: Vec<_> = files
        .into_iter()
        .filter(|entry| entry.path == trimmed_path || entry.path.starts_with(&prefix))
        .collect();
    if entries.is_empty() {
        return Err(CommandError::coded(
            "skill_not_found",
            format!(
                "no files found under {} in {}",
                trimmed_path,
                source.full_name()
            ),
        ));
    }

    let scratch = paths.tmp.join("sync").join(format!(
        "{}-{}-{}",
        source.owner,
        source.repo,
        current_unix_secs()
    ));
    fs::create_dir_all(&scratch).map_err(CommandError::from)?;

    for entry in entries {
        if !matches!(entry.kind, skill_library_provider::FileKind::File) {
            continue;
        }
        let blob = provider
            .read_file(source, git_ref, &entry.path)
            .await
            .map_err(provider_command_error)?;

        let rel = entry
            .path
            .strip_prefix(&prefix)
            .or_else(|| entry.path.strip_prefix(&trimmed_path))
            .unwrap_or(entry.path.as_str())
            .trim_start_matches('/');

        let dest = scratch.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(CommandError::from)?;
        }
        fs::write(&dest, &blob.bytes).map_err(CommandError::from)?;
    }

    if let Some(new_id) = rename_to {
        let new_id = new_id.trim();
        if !new_id.is_empty() {
            rewrite_skill_id(&scratch, new_id)?;
        }
    }

    Ok(scratch)
}

fn rewrite_skill_id(skill_dir: &Path, new_id: &str) -> CommandResult<()> {
    // Rewrite the simplest possible "id: <value>" line in manifest.yaml /
    // SKILL.md frontmatter so package_skill picks up the new id.
    for filename in ["manifest.yaml", "manifest.yml", "SKILL.md"] {
        let path = skill_dir.join(filename);
        if !path.exists() {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(CommandError::from)?;
        let rewritten = raw
            .lines()
            .map(|line| {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("id:") {
                    let leading = &line[..line.len() - trimmed.len()];
                    let _ = rest; // suppress unused warning
                    format!("{leading}id: {new_id}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, rewritten).map_err(CommandError::from)?;
    }
    Ok(())
}

fn bump_version_string(current: &str, bump: &str) -> CommandResult<String> {
    let mut parts = current
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    parts.resize(3, 0);
    let next = match bump {
        "major" => (parts[0] + 1, 0, 0),
        "minor" => (parts[0], parts[1] + 1, 0),
        "patch" => (parts[0], parts[1], parts[2] + 1),
        other => {
            return Err(CommandError::coded(
                "invalid_version_bump",
                format!("unsupported version bump: {other}"),
            ));
        }
    };
    Ok(format!("{}.{}.{}", next.0, next.1, next.2))
}

fn rewrite_yaml_scalar_line(raw: &str, field: &str, value: &str, frontmatter_only: bool) -> String {
    let mut in_frontmatter = !frontmatter_only;
    let mut seen_frontmatter_start = false;
    let mut replaced = false;
    let mut lines = Vec::new();

    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if frontmatter_only && trimmed == "---" {
            if index == 0 && !seen_frontmatter_start {
                seen_frontmatter_start = true;
                in_frontmatter = true;
                lines.push(line.to_owned());
                continue;
            }
            if seen_frontmatter_start && in_frontmatter {
                if !replaced {
                    lines.push(format!("{field}: {value}"));
                    replaced = true;
                }
                in_frontmatter = false;
                lines.push(line.to_owned());
                continue;
            }
        }

        if in_frontmatter {
            let trimmed_start = line.trim_start();
            if trimmed_start
                .strip_prefix(field)
                .and_then(|rest| rest.strip_prefix(':'))
                .is_some()
            {
                let leading = &line[..line.len() - trimmed_start.len()];
                lines.push(format!("{leading}{field}: {value}"));
                replaced = true;
                continue;
            }
        }

        lines.push(line.to_owned());
    }

    if !replaced && !frontmatter_only {
        lines.push(format!("{field}: {value}"));
    }

    let mut rewritten = lines.join("\n");
    if raw.ends_with('\n') {
        rewritten.push('\n');
    }
    rewritten
}

fn rewrite_manifest_json_scalar(path: &Path, field: &str, value: &str) -> CommandResult<()> {
    let raw = fs::read_to_string(path).map_err(CommandError::from)?;
    let mut json: serde_json::Value = serde_json::from_str(&raw).map_err(CommandError::from)?;
    let object = json.as_object_mut().ok_or_else(|| {
        CommandError::coded(
            "invalid_manifest_json",
            format!("{} is not a JSON object", path.display()),
        )
    })?;
    object.insert(
        field.to_owned(),
        serde_json::Value::String(value.to_owned()),
    );
    fs::write(
        path,
        serde_json::to_string_pretty(&json).map_err(CommandError::from)? + "\n",
    )
    .map_err(CommandError::from)
}

fn rewrite_skill_version(skill_dir: &Path, version: &str) -> CommandResult<()> {
    let mut touched = false;
    for filename in ["manifest.yaml", "manifest.yml"] {
        let path = skill_dir.join(filename);
        if !path.exists() {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(CommandError::from)?;
        fs::write(
            &path,
            rewrite_yaml_scalar_line(&raw, "version", version, false),
        )
        .map_err(CommandError::from)?;
        touched = true;
    }

    let json_path = skill_dir.join("manifest.json");
    if json_path.exists() {
        rewrite_manifest_json_scalar(&json_path, "version", version)?;
        touched = true;
    }

    let skill_md_path = skill_dir.join("SKILL.md");
    if skill_md_path.exists() {
        let raw = fs::read_to_string(&skill_md_path).map_err(CommandError::from)?;
        let rewritten = rewrite_yaml_scalar_line(&raw, "version", version, true);
        if rewritten != raw {
            fs::write(&skill_md_path, rewritten).map_err(CommandError::from)?;
            touched = true;
        }
    }

    if touched {
        Ok(())
    } else {
        Err(CommandError::coded(
            "missing_version_target",
            "could not find a manifest or SKILL.md frontmatter to update version",
        ))
    }
}

fn safe_relative_path(path: &Path) -> CommandResult<()> {
    if path.as_os_str().is_empty() {
        return Err(CommandError::coded(
            "invalid_publish_path",
            "publish path cannot be empty",
        ));
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(CommandError::coded(
                "invalid_publish_path",
                format!("unsafe publish path: {}", path.display()),
            ));
        }
    }
    Ok(())
}

fn draft_relative_path(skill_path: &str, file_path: &str) -> CommandResult<PathBuf> {
    let skill_path = skill_path.trim_matches('/');
    let file_path = file_path.trim_matches('/');
    let prefix = format!("{skill_path}/");
    let rel = file_path.strip_prefix(&prefix).ok_or_else(|| {
        CommandError::coded(
            "invalid_publish_draft",
            format!("draft file {file_path} is outside skill path {skill_path}"),
        )
    })?;
    let rel = PathBuf::from(rel);
    safe_relative_path(&rel)?;
    Ok(rel)
}

fn apply_publish_draft(
    scratch: &Path,
    skill_path: &str,
    draft: &PublishDraftInput,
) -> CommandResult<()> {
    let rel = draft_relative_path(skill_path, &draft.file_path)?;
    let dest = scratch.join(rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(CommandError::from)?;
    }
    fs::write(dest, draft.after.as_bytes()).map_err(CommandError::from)
}

fn repo_path_for_skill_file(skill_path: &str, relative_path: &Path) -> CommandResult<String> {
    safe_relative_path(relative_path)?;
    let mut parts = skill_path
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(CommandError::coded(
            "invalid_skill_path",
            "skill path inside the workspace cannot be empty",
        ));
    }
    for component in relative_path.components() {
        let Component::Normal(value) = component else {
            return Err(CommandError::coded(
                "invalid_publish_path",
                format!("unsafe publish path: {}", relative_path.display()),
            ));
        };
        parts.push(value.to_string_lossy().to_string());
    }
    Ok(parts.join("/"))
}

async fn workspace_git_ref(
    provider: &GitHubProvider,
    workspace: &WorkspaceRef,
    source_ref: Option<&str>,
) -> CommandResult<GitRef> {
    match source_ref.map(str::trim).filter(|value| !value.is_empty()) {
        Some(name) => {
            if name.chars().next().map(|c| c == 'v').unwrap_or(false)
                || name.starts_with("refs/tags/")
            {
                Ok(GitRef::Tag(
                    name.trim_start_matches("refs/tags/").to_owned(),
                ))
            } else {
                Ok(GitRef::Branch(name.to_owned()))
            }
        }
        None => Ok(GitRef::Branch(
            provider
                .get_workspace(workspace)
                .await
                .map_err(provider_command_error)?
                .default_branch,
        )),
    }
}

#[tauri::command]
async fn preview_publish_from_workspace(
    source_workspace: String,
    skill_path: String,
    source_ref: Option<String>,
    target_workspace: String,
    rename_to: Option<String>,
    user: Option<String>,
) -> CommandResult<PublishPreview> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before previewing a sync",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    let source_ws = parse_workspace(&source_workspace)?;
    let target_ws = parse_workspace(&target_workspace)?;
    let git_ref = match source_ref
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(name) if name.starts_with("refs/tags/") || !name.contains('/') => {
            // Cheap heuristic: look like a tag (semver-ish) -> Tag, else Branch.
            if name.chars().next().map(|c| c == 'v').unwrap_or(false)
                || name.chars().any(|c| c == '.')
            {
                GitRef::Tag(name.trim_start_matches("refs/tags/").to_owned())
            } else {
                GitRef::Branch(name.to_owned())
            }
        }
        Some(name) => GitRef::Branch(name.to_owned()),
        None => GitRef::Branch(
            provider
                .get_workspace(&source_ws)
                .await
                .map_err(provider_command_error)?
                .default_branch,
        ),
    };

    let scratch = fetch_remote_skill_to_temp(
        &provider,
        &paths,
        &source_ws,
        &skill_path,
        &git_ref,
        rename_to.as_deref(),
    )
    .await?;

    let package = skill_library_publish::package_skill(&scratch)?;
    let policy = skill_library_publish::evaluate_publish_policy(&package)?;
    let request = skill_library_publish::build_publish_request(
        &package,
        &target_ws,
        user.as_deref().unwrap_or("local"),
    );

    Ok(PublishPreview {
        package,
        policy,
        request: Some(request),
    })
}

#[tauri::command]
async fn publish_skill_to_workspace(
    source_workspace: String,
    skill_path: String,
    source_ref: Option<String>,
    target_workspace: String,
    rename_to: Option<String>,
    user: Option<String>,
    confirmed_risk: Option<bool>,
) -> CommandResult<PublishResult> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before publishing",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    let source_ws = parse_workspace(&source_workspace)?;
    let target_ws = parse_workspace(&target_workspace)?;
    let git_ref = match source_ref
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(name) => {
            if name.chars().next().map(|c| c == 'v').unwrap_or(false)
                || name.starts_with("refs/tags/")
            {
                GitRef::Tag(name.trim_start_matches("refs/tags/").to_owned())
            } else {
                GitRef::Branch(name.to_owned())
            }
        }
        None => GitRef::Branch(
            provider
                .get_workspace(&source_ws)
                .await
                .map_err(provider_command_error)?
                .default_branch,
        ),
    };

    let scratch = fetch_remote_skill_to_temp(
        &provider,
        &paths,
        &source_ws,
        &skill_path,
        &git_ref,
        rename_to.as_deref(),
    )
    .await?;

    let package = skill_library_publish::package_skill(&scratch)?;
    let policy = skill_library_publish::evaluate_publish_policy(&package)?;

    if matches!(
        policy.decision,
        skill_library_publish::PublishPolicyDecision::Reject
    ) {
        return Err(CommandError::coded(
            "publish_rejected",
            format!(
                "publish policy rejected this skill: {}",
                policy.reasons.join("; ")
            ),
        ));
    }
    if package.risk_level != skill_library_core::RiskLevel::Low && !confirmed_risk.unwrap_or(false)
    {
        return Err(CommandError::coded(
            "risk_confirmation_required",
            format!(
                "this skill has {} risk; pass confirmedRisk=true to proceed",
                package.risk_level
            ),
        ));
    }

    let request = skill_library_publish::build_publish_request(
        &package,
        &target_ws,
        user.as_deref().unwrap_or("local"),
    );
    let publish_files = skill_library_publish::collect_publish_files(&package)?;
    let github_files: Vec<GitHubPublishFile> = publish_files
        .iter()
        .map(|file| GitHubPublishFile {
            path: file.target_path.clone(),
            bytes: file.bytes.clone(),
        })
        .collect();

    let result = provider
        .publish_files_pull_request(
            &target_ws,
            GitHubPublishInput {
                branch_name: request.branch_name.clone(),
                commit_message: format!(
                    "skill-library: import {} v{}",
                    package.manifest.id, package.manifest.version
                ),
                title: request.title.clone(),
                body: request.body.clone(),
                base: None,
                files: github_files,
            },
        )
        .await
        .map_err(|err| CommandError::coded("publish_failed", err.to_string()))?;
    let pr_number = result.pull_request.number;
    let auto_merge = if policy.auto_merge_allowed
        && can_auto_merge_workspace(&provider, &target_ws).await
    {
        Some(
            try_merge_and_cleanup_branch(&provider, &target_ws, pr_number, &request.branch_name)
                .await,
        )
    } else {
        None
    };

    Ok(PublishResult {
        package,
        policy,
        request,
        pull_request: result.pull_request.into(),
        target_workspace: target_ws.full_name(),
        uploaded_files: result.uploaded.into_iter().map(|f| f.path).collect(),
        auto_merge,
    })
}

#[tauri::command]
async fn publish_workspace_skill_update(
    workspace: String,
    skill_path: String,
    source_ref: Option<String>,
    version_bump: String,
    message: String,
    draft: Option<PublishDraftInput>,
    user: Option<String>,
    confirmed_risk: Option<bool>,
) -> CommandResult<PublishResult> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before publishing",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    let target_ws = parse_workspace(&workspace)?;
    let git_ref = workspace_git_ref(&provider, &target_ws, source_ref.as_deref()).await?;
    let scratch =
        fetch_remote_skill_to_temp(&provider, &paths, &target_ws, &skill_path, &git_ref, None)
            .await?;

    if let Some(draft) = draft.as_ref() {
        apply_publish_draft(&scratch, &skill_path, draft)?;
    }

    let current_package = skill_library_publish::package_skill(&scratch)?;
    let next_version = bump_version_string(&current_package.manifest.version, &version_bump)?;
    rewrite_skill_version(&scratch, &next_version)?;

    let package = skill_library_publish::package_skill(&scratch)?;
    let policy = skill_library_publish::evaluate_publish_policy(&package)?;

    if matches!(
        policy.decision,
        skill_library_publish::PublishPolicyDecision::Reject
    ) {
        return Err(CommandError::coded(
            "publish_rejected",
            format!(
                "publish policy rejected this skill: {}",
                policy.reasons.join("; ")
            ),
        ));
    }
    if package.risk_level != skill_library_core::RiskLevel::Low && !confirmed_risk.unwrap_or(false)
    {
        return Err(CommandError::coded(
            "risk_confirmation_required",
            format!(
                "this skill has {} risk; pass confirmedRisk=true to proceed",
                package.risk_level
            ),
        ));
    }

    let mut request = skill_library_publish::build_publish_request(
        &package,
        &target_ws,
        user.as_deref().unwrap_or("local"),
    );
    request.title = format!(
        "Update skill {} to v{}",
        package.manifest.name, package.manifest.version
    );
    let release_notes = message.trim();
    if !release_notes.is_empty() {
        request.body = format!("## Release notes\n\n{}\n\n{}", release_notes, request.body);
    }

    let publish_files = skill_library_publish::collect_publish_files(&package)?;
    let github_files: Vec<GitHubPublishFile> = publish_files
        .iter()
        .map(|file| {
            repo_path_for_skill_file(&skill_path, &file.relative_path).map(|path| {
                GitHubPublishFile {
                    path,
                    bytes: file.bytes.clone(),
                }
            })
        })
        .collect::<CommandResult<Vec<_>>>()?;

    let result = provider
        .publish_files_pull_request(
            &target_ws,
            GitHubPublishInput {
                branch_name: request.branch_name.clone(),
                commit_message: format!(
                    "skill-library: update {} to v{}",
                    package.manifest.id, package.manifest.version
                ),
                title: request.title.clone(),
                body: request.body.clone(),
                base: None,
                files: github_files,
            },
        )
        .await
        .map_err(|err| CommandError::coded("publish_failed", err.to_string()))?;
    let pr_number = result.pull_request.number;
    let auto_merge = if policy.auto_merge_allowed
        && can_auto_merge_workspace(&provider, &target_ws).await
    {
        Some(
            try_merge_and_cleanup_branch(&provider, &target_ws, pr_number, &request.branch_name)
                .await,
        )
    } else {
        None
    };

    Ok(PublishResult {
        package,
        policy,
        request,
        pull_request: result.pull_request.into(),
        target_workspace: target_ws.full_name(),
        uploaded_files: result.uploaded.into_iter().map(|f| f.path).collect(),
        auto_merge,
    })
}

async fn try_merge_and_cleanup_branch(
    provider: &GitHubProvider,
    workspace: &WorkspaceRef,
    number: u64,
    branch: &str,
) -> PublishAutoMergeResult {
    let merge = provider.merge_pull_request(workspace, number).await;
    match merge {
        Ok(_) => {
            let mut deleted_branch = false;
            let mut error = None;
            if is_skill_library_publish_branch(branch) {
                match provider.delete_branch(workspace, branch).await {
                    Ok(()) => deleted_branch = true,
                    Err(err) => error = Some(format!("merged, but branch cleanup failed: {err}")),
                }
            }
            PublishAutoMergeResult {
                merged: true,
                deleted_branch,
                error,
            }
        }
        Err(err) => PublishAutoMergeResult {
            merged: false,
            deleted_branch: false,
            error: Some(err.to_string()),
        },
    }
}

async fn can_auto_merge_workspace(provider: &GitHubProvider, workspace: &WorkspaceRef) -> bool {
    let user = match provider.current_user().await {
        Ok(user) => user,
        Err(err) => {
            tracing::warn!(target: "skill-library-publish", workspace = %workspace.full_name(), error = %err, "skip auto-merge: unable to read current GitHub user");
            return false;
        }
    };
    match provider.check_permission(workspace, &user.login).await {
        Ok(PermissionLevel::Admin | PermissionLevel::Maintain | PermissionLevel::Write) => true,
        Ok(permission) => {
            tracing::debug!(target: "skill-library-publish", workspace = %workspace.full_name(), user = %user.login, ?permission, "skip auto-merge: insufficient permission");
            false
        }
        Err(err) => {
            tracing::warn!(target: "skill-library-publish", workspace = %workspace.full_name(), user = %user.login, error = %err, "skip auto-merge: permission check failed");
            false
        }
    }
}

fn is_skill_library_publish_branch(branch: &str) -> bool {
    let parts = branch.split('/').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != "skill-library" || parts[1] != "import" {
        return false;
    }
    let skill = parts[2];
    let hash = parts[3];
    !skill.is_empty()
        && skill
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && hash.len() == 12
        && hash.chars().all(|c| c.is_ascii_hexdigit())
}

#[tauri::command]
fn export_diagnostics() -> CommandResult<DiagnosticsExport> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    export_diagnostics_bundle(&paths)
}

#[tauri::command]
fn open_logs_folder(app: tauri::AppHandle) -> CommandResult<()> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    app.opener()
        .open_path(paths.logs.to_string_lossy().to_string(), None::<&str>)
        .map_err(|err| CommandError::coded("open_logs_failed", err.to_string()))?;
    Ok(())
}

fn export_diagnostics_bundle(paths: &AppPaths) -> CommandResult<DiagnosticsExport> {
    let exported_at = chrono::Utc::now();
    let output_dir = paths
        .tmp
        .join("diagnostics")
        .join(exported_at.format("%Y%m%dT%H%M%SZ").to_string());
    fs::create_dir_all(&output_dir).map_err(CommandError::from)?;
    let subscriptions = skill_library_sync::read_subscriptions(&paths.subscriptions)?;
    let workspaces = skill_library_sync::read_workspaces(&paths.workspace_registry)?;
    fs::write(
        output_dir.join("summary.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "exportedAt": exported_at,
            "appHome": paths.home,
            "subscriptionCount": subscriptions.subscriptions.len(),
            "workspaceCount": workspaces.workspaces.len(),
        }))
        .map_err(CommandError::from)?,
    )
    .map_err(CommandError::from)?;
    fs::write(
        output_dir.join("subscriptions.json"),
        serde_json::to_vec_pretty(&subscriptions).map_err(CommandError::from)?,
    )
    .map_err(CommandError::from)?;
    fs::write(
        output_dir.join("workspaces.json"),
        serde_json::to_vec_pretty(&workspaces).map_err(CommandError::from)?,
    )
    .map_err(CommandError::from)?;

    let logs = copy_sanitized_logs(&paths.logs, &output_dir.join("logs"))?;
    let export = DiagnosticsExport {
        exported_at: exported_at.to_rfc3339(),
        output_dir,
        app_home: paths.home.clone(),
        subscriptions: subscriptions.subscriptions.len(),
        workspaces: workspaces.workspaces.len(),
        logs,
        notes: vec![
            "credentials.json and OS keychain secrets are intentionally excluded".to_owned(),
            "log files are copied with token-looking values redacted".to_owned(),
        ],
    };
    fs::write(
        export.output_dir.join("diagnostics.json"),
        serde_json::to_vec_pretty(&export).map_err(CommandError::from)?,
    )
    .map_err(CommandError::from)?;
    Ok(export)
}

fn copy_sanitized_logs(logs_dir: &Path, output_dir: &Path) -> CommandResult<Vec<PathBuf>> {
    if !logs_dir.exists() {
        return Ok(Vec::new());
    }
    fs::create_dir_all(output_dir).map_err(CommandError::from)?;
    let mut copied = Vec::new();
    for entry in fs::read_dir(logs_dir).map_err(CommandError::from)? {
        let source = entry.map_err(CommandError::from)?.path();
        if !source.is_file() || source.extension().and_then(|value| value.to_str()) != Some("log") {
            continue;
        }
        let Some(file_name) = source.file_name() else {
            continue;
        };
        let destination = output_dir.join(file_name);
        let raw = fs::read_to_string(&source).unwrap_or_else(|_| "<binary log omitted>".to_owned());
        fs::write(&destination, redact_sensitive_text(&raw)).map_err(CommandError::from)?;
        copied.push(destination);
    }
    copied.sort();
    Ok(copied)
}

fn redact_sensitive_text(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"GITHUB_TOKEN") {
            output.extend_from_slice(b"[REDACTED]");
            index += b"GITHUB_TOKEN".len();
            continue;
        }
        if let Some(prefix) = github_token_prefix(&bytes[index..]) {
            output.extend_from_slice(b"[REDACTED]");
            index += prefix.len();
            while index < bytes.len() && is_github_token_char(bytes[index]) {
                index += 1;
            }
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(output).unwrap_or_else(|_| "[REDACTED]".to_owned())
}

fn github_token_prefix(value: &[u8]) -> Option<&'static [u8]> {
    const PREFIXES: &[&[u8]] = &[b"github_pat_", b"ghp_", b"gho_", b"ghu_", b"ghs_", b"ghr_"];
    PREFIXES
        .iter()
        .copied()
        .find(|prefix| value.starts_with(prefix))
}

fn is_github_token_char(value: u8) -> bool {
    value.is_ascii_alphanumeric() || value == b'_' || value == b'-'
}

fn register_deep_link<R: tauri::Runtime>(app: &tauri::AppHandle<R>, url: Url) {
    let payload = parse_deep_link(url);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focus();
        let _ = window.show();
        let _ = window.unminimize();
    }
    if let Some(payload) = payload {
        if let Some(state) = app.try_state::<DeepLinkState>() {
            *state.last.lock().unwrap() = Some(payload.clone());
        }
        let _ = app.emit(DEEP_LINK_EVENT, payload);
    }
}

fn parse_deep_link(url: Url) -> Option<DeepLinkPayload> {
    if url.scheme() != "skill-library" {
        return None;
    }
    let action = url.host_str().unwrap_or_default().to_owned();
    if action != DEEP_LINK_SUBSCRIBE_PATH {
        return Some(DeepLinkPayload {
            url: url.to_string(),
            action,
            workspace: None,
            asset_id: None,
            version: None,
            targets: Vec::new(),
            query: parse_query_pairs(&url),
        });
    }

    let query = parse_query_pairs(&url);
    let workspace = query
        .get("workspace")
        .and_then(|value| parse_workspace(value).ok());
    let asset_id = query
        .get("assetId")
        .cloned()
        .or_else(|| query.get("asset_id").cloned());
    let version = query.get("version").cloned();
    let targets = query
        .get("targets")
        .map(|value| {
            value
                .split(',')
                .map(|target| target.trim().to_owned())
                .filter(|target| !target.is_empty())
                .collect()
        })
        .unwrap_or_default();

    Some(DeepLinkPayload {
        url: url.to_string(),
        action,
        workspace,
        asset_id,
        version,
        targets,
        query,
    })
}

fn parse_query_pairs(url: &Url) -> HashMap<String, String> {
    url.query_pairs()
        .into_owned()
        .collect::<HashMap<String, String>>()
}

fn parse_workspace(value: &str) -> CommandResult<WorkspaceRef> {
    let value = value
        .trim()
        .strip_prefix("github.com/")
        .unwrap_or_else(|| value.trim());
    let Some((owner, repo)) = value.split_once('/') else {
        return Err(CommandError::coded(
            "invalid_workspace",
            "workspace must look like owner/repo or github.com/owner/repo",
        ));
    };
    Ok(WorkspaceRef::github(owner, repo))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceHeadInfo {
    sha: String,
    branch: String,
    committed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceChangedPaths {
    base_sha: String,
    head_sha: String,
    changed_skill_paths: Vec<String>,
    total_changed_files: usize,
}

/// Returns the HEAD commit SHA of the workspace's default branch.
/// This is the cheapest possible check — one API call to detect any change.
#[tauri::command]
async fn check_workspace_head(workspace: String) -> CommandResult<WorkspaceHeadInfo> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before checking workspace head",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;

    // Get workspace info for default branch name
    let ws_info = provider
        .get_workspace(&workspace)
        .await
        .map_err(provider_command_error)?;

    // Get the latest commit on the default branch (path="" = repo root)
    let commits = provider
        .list_path_commits(&workspace, "", Some(&ws_info.default_branch), 1)
        .await
        .map_err(provider_command_error)?;

    let head = commits
        .first()
        .ok_or_else(|| CommandError::coded("no_commits", "workspace has no commits"))?;

    Ok(WorkspaceHeadInfo {
        sha: head.sha.clone(),
        branch: ws_info.default_branch,
        committed_at: Some(head.authored_at.clone()),
    })
}

/// Compares two SHAs and returns which skill paths were affected.
/// Only called when check_workspace_head detects a SHA change.
#[tauri::command]
async fn diff_workspace_since(
    workspace: String,
    base_sha: String,
    head_sha: String,
    skill_paths: Vec<String>,
) -> CommandResult<WorkspaceChangedPaths> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before diffing workspace",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;

    // GitHub compare API accepts SHAs as branch refs
    let comparison = provider
        .compare_refs(
            &workspace,
            &GitRef::Branch(base_sha.clone()),
            &GitRef::Branch(head_sha.clone()),
        )
        .await
        .map_err(provider_command_error)?;

    let total_changed_files = comparison.files.len();

    // Find which known skill paths have changed files
    let changed_skill_paths: Vec<String> = skill_paths
        .into_iter()
        .filter(|skill_path| {
            let prefix = skill_path.trim_matches('/');
            comparison.files.iter().any(|file| {
                file.filename.starts_with(prefix)
                    || file.filename.starts_with(&format!("{prefix}/"))
            })
        })
        .collect();

    Ok(WorkspaceChangedPaths {
        base_sha,
        head_sha,
        changed_skill_paths,
        total_changed_files,
    })
}

// ---------------------------------------------------------------------------
// Branch listing
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BranchInfo {
    name: String,
    is_default: bool,
}

/// Lists branches for a workspace repository.
#[tauri::command]
async fn list_workspace_branches(workspace: String) -> CommandResult<Vec<BranchInfo>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths)
        .ok_or_else(|| CommandError::coded("missing_github_token", "log in with GitHub first"))?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;

    let ws_info = provider
        .get_workspace(&workspace)
        .await
        .map_err(provider_command_error)?;
    let branches = provider
        .list_branches(&workspace)
        .await
        .map_err(provider_command_error)?;

    Ok(branches
        .into_iter()
        .map(|name| BranchInfo {
            is_default: name == ws_info.default_branch,
            name,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Skill file tree & single file read
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillFileEntry {
    path: String,
    relative_path: String,
    kind: String, // "file" | "directory"
    size: Option<u64>,
}

/// Lists all files inside a skill directory (recursive).
#[tauri::command]
async fn list_skill_files(
    workspace: String,
    skill_path: String,
    ref_name: Option<String>,
) -> CommandResult<Vec<SkillFileEntry>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths);
    let provider = github_provider(token.as_deref())?;

    let git_ref = match ref_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => GitRef::Branch(name.to_owned()),
        None => {
            let ws_info = provider
                .get_workspace(&workspace)
                .await
                .map_err(provider_command_error)?;
            GitRef::Branch(ws_info.default_branch)
        }
    };

    let all_files = provider
        .list_files(&workspace, &git_ref)
        .await
        .map_err(provider_command_error)?;

    let prefix = skill_path.trim_matches('/');
    let prefix_with_slash = format!("{prefix}/");

    let entries: Vec<SkillFileEntry> = all_files
        .into_iter()
        .filter(|entry| entry.path.starts_with(&prefix_with_slash))
        .map(|entry| {
            let relative_path = entry.path[prefix_with_slash.len()..].to_owned();
            SkillFileEntry {
                path: entry.path,
                relative_path,
                kind: match entry.kind {
                    skill_library_provider::FileKind::Directory => "directory".to_owned(),
                    _ => "file".to_owned(),
                },
                size: entry.size,
            }
        })
        .collect();

    Ok(entries)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileContent {
    path: String,
    content: String,
    sha: String,
    encoding: String,
    is_binary: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillPackageCacheResult {
    workspace: String,
    skill_path: String,
    ref_name: String,
    file_count: usize,
    cached_count: usize,
    skipped_count: usize,
    total_bytes: u64,
}

/// Reads a single file from the workspace repo.
#[tauri::command]
async fn read_skill_file(
    workspace: String,
    file_path: String,
    ref_name: Option<String>,
) -> CommandResult<FileContent> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths);
    let provider = github_provider(token.as_deref())?;

    let git_ref = match ref_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => GitRef::Branch(name.to_owned()),
        None => {
            let ws_info = provider
                .get_workspace(&workspace)
                .await
                .map_err(provider_command_error)?;
            GitRef::Branch(ws_info.default_branch)
        }
    };

    let blob = provider
        .read_file(&workspace, &git_ref, &file_path)
        .await
        .map_err(provider_command_error)?;

    // Try to decode as UTF-8; if it fails, mark as binary
    let (content, is_binary) = match String::from_utf8(blob.bytes.clone()) {
        Ok(text) => (text, false),
        Err(_) => (
            base64::engine::general_purpose::STANDARD.encode(&blob.bytes),
            true,
        ),
    };

    Ok(FileContent {
        path: blob.path,
        content,
        sha: blob.sha,
        encoding: if is_binary {
            "base64".to_owned()
        } else {
            "utf-8".to_owned()
        },
        is_binary,
    })
}

/// Warms the local remote cache with all files inside a skill directory.
#[tauri::command]
async fn cache_skill_package(
    workspace: String,
    skill_path: String,
    ref_name: Option<String>,
) -> CommandResult<SkillPackageCacheResult> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace_ref = parse_workspace(&workspace)?;
    let workspace_label = workspace_ref.full_name();
    let token = saved_github_token(&paths);
    let provider = github_provider(token.as_deref())?;

    let (git_ref, cache_ref) = match ref_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => (GitRef::Branch(name.to_owned()), name.to_owned()),
        None => {
            let ws_info = provider
                .get_workspace(&workspace_ref)
                .await
                .map_err(provider_command_error)?;
            (GitRef::Branch(ws_info.default_branch), "HEAD".to_owned())
        }
    };

    let all_files = provider
        .list_files(&workspace_ref, &git_ref)
        .await
        .map_err(provider_command_error)?;

    let prefix = skill_path.trim_matches('/');
    let prefix_with_slash = format!("{prefix}/");
    let file_paths: Vec<String> = all_files
        .into_iter()
        .filter(|entry| entry.path.starts_with(&prefix_with_slash))
        .filter(|entry| {
            matches!(
                entry.kind,
                skill_library_provider::FileKind::File | skill_library_provider::FileKind::Symlink
            )
        })
        .map(|entry| entry.path)
        .collect();

    let mut cached_count = 0usize;
    let mut skipped_count = 0usize;
    let mut total_bytes = 0u64;

    for file_path in &file_paths {
        let target = build_cache_path(&workspace_label, &cache_ref, file_path)?;
        if target.exists() {
            skipped_count += 1;
            if let Ok(meta) = fs::metadata(&target) {
                total_bytes = total_bytes.saturating_add(meta.len());
            }
            continue;
        }

        let blob = provider
            .read_file(&workspace_ref, &git_ref, file_path)
            .await
            .map_err(provider_command_error)?;
        total_bytes = total_bytes.saturating_add(blob.bytes.len() as u64);
        write_remote_cache_file_bytes(&workspace_label, &cache_ref, &blob.path, &blob.bytes)?;
        cached_count += 1;
    }

    Ok(SkillPackageCacheResult {
        workspace: workspace_label,
        skill_path: prefix.to_owned(),
        ref_name: cache_ref,
        file_count: file_paths.len(),
        cached_count,
        skipped_count,
        total_bytes,
    })
}

// ---------------------------------------------------------------------------
// GitHub Discussions (likes + comments)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscussionInfo {
    id: String,
    number: u64,
    title: String,
    url: String,
    body: String,
    body_author: String,
    body_author_avatar: Option<String>,
    upvotes: u64,
    comment_count: u64,
    created_at: String,
    has_discussions: bool,
    reactions: Vec<ReactionGroup>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReactionGroup {
    content: String,
    count: u64,
    viewer_has_reacted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscussionComment {
    id: String,
    author: String,
    author_avatar: Option<String>,
    body: String,
    created_at: String,
    upvotes: u64,
    reactions: Vec<ReactionGroup>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscussionsStatus {
    enabled: bool,
    discussions: Vec<DiscussionInfo>,
}

/// Check if Discussions are enabled and list skill discussions.
#[tauri::command]
async fn list_skill_discussions(
    workspace: String,
    skill_ids: Vec<String>,
) -> CommandResult<DiscussionsStatus> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths)
        .ok_or_else(|| CommandError::coded("missing_github_token", "log in with GitHub first"))?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    // Use GraphQL to check if discussions are enabled and fetch them
    #[derive(serde::Deserialize)]
    struct GqlResponse {
        repository: Option<GqlRepo>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlRepo {
        has_discussions_enabled: bool,
        discussions: Option<GqlDiscussionConnection>,
    }
    #[derive(serde::Deserialize)]
    struct GqlDiscussionConnection {
        nodes: Vec<GqlDiscussion>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlDiscussion {
        id: String,
        number: u64,
        title: String,
        url: String,
        body: String,
        created_at: String,
        upvote_count: u64,
        author: Option<GqlAuthor>,
        comments: GqlCommentCount,
        reaction_groups: Option<Vec<GqlReactionGroup>>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlAuthor {
        login: String,
        avatar_url: Option<String>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlCommentCount {
        total_count: u64,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactionGroup {
        content: String,
        reactors: GqlReactorConnection,
        viewer_has_reacted: bool,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactorConnection {
        total_count: u64,
    }

    let query = format!(
        r#"query {{
            repository(owner: "{}", name: "{}") {{
                hasDiscussionsEnabled
                discussions(first: 100, orderBy: {{field: CREATED_AT, direction: DESC}}) {{
                    nodes {{
                        id
                        number
                        title
                        url
                        body
                        createdAt
                        upvoteCount
                        author {{ login avatarUrl }}
                        comments {{ totalCount }}
                        reactionGroups {{
                            content
                            reactors {{ totalCount }}
                            viewerHasReacted
                        }}
                    }}
                }}
            }}
        }}"#,
        workspace.owner, workspace.repo
    );

    let result: GqlResponse = provider
        .graphql(&query, serde_json::json!({}))
        .await
        .map_err(provider_command_error)?;

    let Some(repo) = result.repository else {
        return Ok(DiscussionsStatus {
            enabled: false,
            discussions: Vec::new(),
        });
    };

    if !repo.has_discussions_enabled {
        return Ok(DiscussionsStatus {
            enabled: false,
            discussions: Vec::new(),
        });
    }

    let discussions: Vec<DiscussionInfo> = repo
        .discussions
        .map(|conn| conn.nodes)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| {
            // Match discussions that are tagged with [skill] prefix
            let title_lower = d.title.to_lowercase();
            skill_ids.iter().any(|id| {
                title_lower.contains(&format!("[skill] {}", id.to_lowercase()))
                    || title_lower.contains(&format!("[skill]{}", id.to_lowercase()))
            })
        })
        .map(|d| {
            let reactions: Vec<ReactionGroup> = d
                .reaction_groups
                .unwrap_or_default()
                .into_iter()
                .filter(|r| r.reactors.total_count > 0)
                .map(|r| ReactionGroup {
                    content: r.content,
                    count: r.reactors.total_count,
                    viewer_has_reacted: r.viewer_has_reacted,
                })
                .collect();
            DiscussionInfo {
                id: d.id,
                number: d.number,
                title: d.title,
                url: d.url,
                body: d.body,
                body_author: d
                    .author
                    .as_ref()
                    .map(|a| a.login.clone())
                    .unwrap_or_else(|| "ghost".to_owned()),
                body_author_avatar: d.author.and_then(|a| a.avatar_url),
                upvotes: d.upvote_count,
                comment_count: d.comments.total_count,
                created_at: d.created_at,
                has_discussions: true,
                reactions,
            }
        })
        .collect();

    Ok(DiscussionsStatus {
        enabled: true,
        discussions,
    })
}

/// Get a single discussion by number (used with cached mapping to skip full scan).
#[tauri::command]
async fn get_discussion_by_number(
    workspace: String,
    discussion_number: u64,
) -> CommandResult<Option<DiscussionInfo>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths)
        .ok_or_else(|| CommandError::coded("missing_github_token", "log in with GitHub first"))?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    #[derive(serde::Deserialize)]
    struct GqlResponse {
        repository: Option<GqlRepo>,
    }
    #[derive(serde::Deserialize)]
    struct GqlRepo {
        discussion: Option<GqlDiscussion>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlDiscussion {
        id: String,
        number: u64,
        title: String,
        url: String,
        body: String,
        created_at: String,
        upvote_count: u64,
        author: Option<GqlAuthor>,
        comments: GqlCommentCount,
        reaction_groups: Option<Vec<GqlReactionGroup>>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlAuthor {
        login: String,
        avatar_url: Option<String>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlCommentCount {
        total_count: u64,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactionGroup {
        content: String,
        reactors: GqlReactorConnection,
        viewer_has_reacted: bool,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactorConnection {
        total_count: u64,
    }

    let query = format!(
        r#"query {{
            repository(owner: "{}", name: "{}") {{
                discussion(number: {}) {{
                    id
                    number
                    title
                    url
                    body
                    createdAt
                    upvoteCount
                    author {{ login avatarUrl }}
                    comments {{ totalCount }}
                    reactionGroups {{
                        content
                        reactors {{ totalCount }}
                        viewerHasReacted
                    }}
                }}
            }}
        }}"#,
        workspace.owner, workspace.repo, discussion_number
    );

    let result: GqlResponse = provider
        .graphql(&query, serde_json::json!({}))
        .await
        .map_err(provider_command_error)?;

    let info = result.repository.and_then(|r| r.discussion).map(|d| {
        let reactions: Vec<ReactionGroup> = d
            .reaction_groups
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.reactors.total_count > 0)
            .map(|r| ReactionGroup {
                content: r.content,
                count: r.reactors.total_count,
                viewer_has_reacted: r.viewer_has_reacted,
            })
            .collect();
        DiscussionInfo {
            id: d.id,
            number: d.number,
            title: d.title,
            url: d.url,
            body: d.body,
            body_author: d
                .author
                .as_ref()
                .map(|a| a.login.clone())
                .unwrap_or_else(|| "ghost".to_owned()),
            body_author_avatar: d.author.and_then(|a| a.avatar_url),
            upvotes: d.upvote_count,
            comment_count: d.comments.total_count,
            created_at: d.created_at,
            has_discussions: true,
            reactions,
        }
    });

    Ok(info)
}

/// Get comments for a specific discussion.
#[tauri::command]
async fn get_discussion_comments(
    workspace: String,
    discussion_number: u64,
) -> CommandResult<Vec<DiscussionComment>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths)
        .ok_or_else(|| CommandError::coded("missing_github_token", "log in with GitHub first"))?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    #[derive(serde::Deserialize)]
    struct GqlResponse {
        repository: Option<GqlRepo>,
    }
    #[derive(serde::Deserialize)]
    struct GqlRepo {
        discussion: Option<GqlDiscussion>,
    }
    #[derive(serde::Deserialize)]
    struct GqlDiscussion {
        comments: GqlCommentConnection,
    }
    #[derive(serde::Deserialize)]
    struct GqlCommentConnection {
        nodes: Vec<GqlComment>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlComment {
        id: String,
        body: String,
        created_at: String,
        upvote_count: u64,
        author: Option<GqlAuthor>,
        reaction_groups: Option<Vec<GqlReactionGroup>>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlAuthor {
        login: String,
        avatar_url: Option<String>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactionGroup {
        content: String,
        reactors: GqlReactorConnection,
        viewer_has_reacted: bool,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactorConnection {
        total_count: u64,
    }

    let query = format!(
        r#"query {{
            repository(owner: "{}", name: "{}") {{
                discussion(number: {}) {{
                    comments(first: 50) {{
                        nodes {{
                            id
                            body
                            createdAt
                            upvoteCount
                            author {{ login avatarUrl }}
                            reactionGroups {{
                                content
                                reactors {{ totalCount }}
                                viewerHasReacted
                            }}
                        }}
                    }}
                }}
            }}
        }}"#,
        workspace.owner, workspace.repo, discussion_number
    );

    let result: GqlResponse = provider
        .graphql(&query, serde_json::json!({}))
        .await
        .map_err(provider_command_error)?;

    let comments = result
        .repository
        .and_then(|r| r.discussion)
        .map(|d| d.comments.nodes)
        .unwrap_or_default()
        .into_iter()
        .map(|c| {
            let reactions: Vec<ReactionGroup> = c
                .reaction_groups
                .unwrap_or_default()
                .into_iter()
                .filter(|r| r.reactors.total_count > 0)
                .map(|r| ReactionGroup {
                    content: r.content,
                    count: r.reactors.total_count,
                    viewer_has_reacted: r.viewer_has_reacted,
                })
                .collect();
            DiscussionComment {
                id: c.id,
                author: c
                    .author
                    .as_ref()
                    .map(|a| a.login.clone())
                    .unwrap_or_else(|| "ghost".to_owned()),
                author_avatar: c.author.and_then(|a| a.avatar_url),
                body: c.body,
                created_at: c.created_at,
                upvotes: c.upvote_count,
                reactions,
            }
        })
        .collect();

    Ok(comments)
}

/// Add a comment to a discussion (using GraphQL mutation).
#[tauri::command]
async fn add_discussion_comment(
    workspace: String,
    discussion_id: String,
    body: String,
) -> CommandResult<DiscussionComment> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let _workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths)
        .ok_or_else(|| CommandError::coded("missing_github_token", "log in with GitHub first"))?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlResponse {
        add_discussion_comment: Option<GqlMutation>,
    }
    #[derive(serde::Deserialize)]
    struct GqlMutation {
        comment: Option<GqlComment>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlComment {
        id: String,
        body: String,
        created_at: String,
        author: Option<GqlAuthor>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlAuthor {
        login: String,
        avatar_url: Option<String>,
    }

    let escaped_body = body
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let query = format!(
        r#"mutation {{
            addDiscussionComment(input: {{discussionId: "{discussion_id}", body: "{escaped_body}"}}) {{
                comment {{
                    id
                    body
                    createdAt
                    author {{ login avatarUrl }}
                }}
            }}
        }}"#
    );

    let result: GqlResponse = provider
        .graphql(&query, serde_json::json!({}))
        .await
        .map_err(provider_command_error)?;

    let comment = result
        .add_discussion_comment
        .and_then(|m| m.comment)
        .ok_or_else(|| CommandError::coded("discussion_error", "failed to add comment"))?;

    Ok(DiscussionComment {
        id: comment.id,
        author: comment
            .author
            .as_ref()
            .map(|a| a.login.clone())
            .unwrap_or_else(|| "ghost".to_owned()),
        author_avatar: comment.author.and_then(|a| a.avatar_url),
        body: comment.body,
        created_at: comment.created_at,
        upvotes: 0,
        reactions: Vec::new(),
    })
}

/// Toggle a reaction on a discussion (using GraphQL mutation).
/// Supported content values: THUMBS_UP, THUMBS_DOWN, LAUGH, HOORAY, CONFUSED, HEART, ROCKET, EYES
#[tauri::command]
async fn toggle_discussion_reaction(
    workspace: String,
    discussion_id: String,
    content: String,
) -> CommandResult<Vec<ReactionGroup>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let _workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths)
        .ok_or_else(|| CommandError::coded("missing_github_token", "log in with GitHub first"))?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    // Validate reaction content
    let valid_reactions = [
        "THUMBS_UP",
        "THUMBS_DOWN",
        "LAUGH",
        "HOORAY",
        "CONFUSED",
        "HEART",
        "ROCKET",
        "EYES",
    ];
    if !valid_reactions.contains(&content.as_str()) {
        return Err(CommandError::coded(
            "invalid_reaction",
            &format!("invalid reaction: {}", content),
        ));
    }

    // Try to add the reaction first
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AddResponse {
        add_reaction: Option<AddReactionPayload>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AddReactionPayload {
        subject: Option<ReactionSubject>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ReactionSubject {
        reaction_groups: Vec<GqlReactionGroup2>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactionGroup2 {
        content: String,
        reactors: GqlReactorCount,
        viewer_has_reacted: bool,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactorCount {
        total_count: u64,
    }

    // First try addReaction — if viewer already reacted, use removeReaction
    let add_query = format!(
        r#"mutation {{
            addReaction(input: {{subjectId: "{discussion_id}", content: {content}}}) {{
                subject {{
                    ... on Discussion {{
                        reactionGroups {{
                            content
                            reactors {{ totalCount }}
                            viewerHasReacted
                        }}
                    }}
                }}
            }}
        }}"#
    );

    let add_result: AddResponse = provider
        .graphql(&add_query, serde_json::json!({}))
        .await
        .map_err(provider_command_error)?;

    // Check if we need to remove instead (viewer already had this reaction)
    // GitHub's addReaction is idempotent — if already reacted, it returns the existing state.
    // We need to check viewerHasReacted to decide if we should remove.
    if let Some(subject) = add_result.add_reaction.and_then(|a| a.subject) {
        // Check if viewer already had this reaction before we added it
        // Since addReaction is idempotent, we check if viewerHasReacted is true
        // and the count didn't change — but we can't easily detect that.
        // Instead, we'll just return the current state after add.
        // For toggle behavior: we always add first. If user clicks again, we remove.
        let groups: Vec<ReactionGroup> = subject
            .reaction_groups
            .into_iter()
            .filter(|r| r.reactors.total_count > 0)
            .map(|r| ReactionGroup {
                content: r.content,
                count: r.reactors.total_count,
                viewer_has_reacted: r.viewer_has_reacted,
            })
            .collect();
        return Ok(groups);
    }

    Ok(Vec::new())
}

/// Remove a reaction from a discussion.
#[tauri::command]
async fn remove_discussion_reaction(
    workspace: String,
    discussion_id: String,
    content: String,
) -> CommandResult<Vec<ReactionGroup>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let _workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths)
        .ok_or_else(|| CommandError::coded("missing_github_token", "log in with GitHub first"))?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RemoveResponse {
        remove_reaction: Option<RemoveReactionPayload>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RemoveReactionPayload {
        subject: Option<ReactionSubject>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ReactionSubject {
        reaction_groups: Vec<GqlReactionGroup2>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactionGroup2 {
        content: String,
        reactors: GqlReactorCount,
        viewer_has_reacted: bool,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GqlReactorCount {
        total_count: u64,
    }

    let remove_query = format!(
        r#"mutation {{
            removeReaction(input: {{subjectId: "{discussion_id}", content: {content}}}) {{
                subject {{
                    ... on Discussion {{
                        reactionGroups {{
                            content
                            reactors {{ totalCount }}
                            viewerHasReacted
                        }}
                    }}
                }}
            }}
        }}"#
    );

    let remove_result: RemoveResponse = provider
        .graphql(&remove_query, serde_json::json!({}))
        .await
        .map_err(provider_command_error)?;

    if let Some(subject) = remove_result.remove_reaction.and_then(|r| r.subject) {
        let groups: Vec<ReactionGroup> = subject
            .reaction_groups
            .into_iter()
            .filter(|r| r.reactors.total_count > 0)
            .map(|r| ReactionGroup {
                content: r.content,
                count: r.reactors.total_count,
                viewer_has_reacted: r.viewer_has_reacted,
            })
            .collect();
        return Ok(groups);
    }

    Ok(Vec::new())
}

/// Create a discussion for a skill (with race-condition re-check).
/// If a discussion already exists, returns the existing one instead of creating a duplicate.
#[tauri::command]
async fn create_skill_discussion(
    workspace: String,
    skill_id: String,
    skill_path: Option<String>,
    body: Option<String>,
) -> CommandResult<DiscussionInfo> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths)
        .ok_or_else(|| CommandError::coded("missing_github_token", "log in with GitHub first"))?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;

    let expected_title = format!("[skill] {}", skill_id);

    // Step 1: Re-check if discussion already exists (race condition guard)
    #[derive(serde::Deserialize)]
    struct CheckResponse {
        repository: Option<CheckRepo>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CheckRepo {
        id: String,
        has_discussions_enabled: bool,
        discussions: Option<CheckDiscussionConn>,
        discussion_categories: Option<CheckCategoryConn>,
    }
    #[derive(serde::Deserialize)]
    struct CheckDiscussionConn {
        nodes: Vec<CheckDiscussion>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CheckDiscussion {
        id: String,
        number: u64,
        title: String,
        url: String,
        created_at: String,
        upvote_count: u64,
        comments: CheckCommentCount,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CheckCommentCount {
        total_count: u64,
    }
    #[derive(serde::Deserialize)]
    struct CheckCategoryConn {
        nodes: Vec<CheckCategory>,
    }
    #[derive(serde::Deserialize)]
    struct CheckCategory {
        id: String,
        name: String,
    }

    let check_query = format!(
        r#"query {{
            repository(owner: "{}", name: "{}") {{
                id
                hasDiscussionsEnabled
                discussions(first: 100, orderBy: {{field: CREATED_AT, direction: DESC}}) {{
                    nodes {{
                        id
                        number
                        title
                        url
                        createdAt
                        upvoteCount
                        comments {{ totalCount }}
                    }}
                }}
                discussionCategories(first: 20) {{
                    nodes {{ id name }}
                }}
            }}
        }}"#,
        workspace.owner, workspace.repo
    );

    let check_result: CheckResponse = provider
        .graphql(&check_query, serde_json::json!({}))
        .await
        .map_err(provider_command_error)?;

    let repo = check_result
        .repository
        .ok_or_else(|| CommandError::coded("repo_not_found", "repository not found"))?;

    if !repo.has_discussions_enabled {
        return Err(CommandError::coded(
            "discussions_disabled",
            "Discussions are not enabled for this repository",
        ));
    }

    // Check if discussion already exists
    let title_lower = expected_title.to_lowercase();
    if let Some(existing) = repo.discussions.as_ref().and_then(|conn| {
        conn.nodes
            .iter()
            .find(|d| d.title.to_lowercase() == title_lower)
    }) {
        // Already exists — return it (race condition resolved)
        return Ok(DiscussionInfo {
            id: existing.id.clone(),
            number: existing.number,
            title: existing.title.clone(),
            url: existing.url.clone(),
            body: String::new(),
            body_author: "ghost".to_owned(),
            body_author_avatar: None,
            upvotes: existing.upvote_count,
            comment_count: existing.comments.total_count,
            created_at: existing.created_at.clone(),
            has_discussions: true,
            reactions: Vec::new(),
        });
    }

    // Step 2: Find a suitable category (prefer "General", fallback to first)
    let categories = repo
        .discussion_categories
        .map(|c| c.nodes)
        .unwrap_or_default();

    let category_id = categories
        .iter()
        .find(|c| c.name.to_lowercase() == "general")
        .or_else(|| categories.first())
        .map(|c| c.id.clone())
        .ok_or_else(|| {
            CommandError::coded(
                "no_category",
                "no discussion categories found — create at least one category in GitHub settings",
            )
        })?;

    // Step 3: Create the discussion
    let repo_id = repo.id;
    let skill_url = if let Some(ref path) = skill_path {
        format!(
            "https://github.com/{}/{}/tree/main/{}",
            workspace.owner,
            workspace.repo,
            path.trim_matches('/')
        )
    } else {
        format!("https://github.com/{}/{}", workspace.owner, workspace.repo)
    };
    let discussion_body = body.unwrap_or_else(|| {
        format!(
            "Discussion for skill [`{}`]({}).\n\nFeel free to share feedback, questions, or suggestions.",
            skill_id, skill_url
        )
    });
    let escaped_body = discussion_body
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    let escaped_title = expected_title.replace('\\', "\\\\").replace('"', "\\\"");

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CreateResponse {
        create_discussion: Option<CreateMutation>,
    }
    #[derive(serde::Deserialize)]
    struct CreateMutation {
        discussion: Option<CreatedDiscussion>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CreatedDiscussion {
        id: String,
        number: u64,
        title: String,
        url: String,
        created_at: String,
    }

    let create_query = format!(
        r#"mutation {{
            createDiscussion(input: {{repositoryId: "{repo_id}", categoryId: "{category_id}", title: "{escaped_title}", body: "{escaped_body}"}}) {{
                discussion {{
                    id
                    number
                    title
                    url
                    createdAt
                }}
            }}
        }}"#
    );

    let create_result: CreateResponse = provider
        .graphql(&create_query, serde_json::json!({}))
        .await
        .map_err(provider_command_error)?;

    let created = create_result
        .create_discussion
        .and_then(|m| m.discussion)
        .ok_or_else(|| {
            CommandError::coded(
                "create_failed",
                "failed to create discussion — you may not have write access",
            )
        })?;

    Ok(DiscussionInfo {
        id: created.id,
        number: created.number,
        title: created.title,
        url: created.url,
        body: String::new(),
        body_author: "ghost".to_owned(),
        body_author_avatar: None,
        upvotes: 0,
        comment_count: 0,
        created_at: created.created_at,
        has_discussions: true,
        reactions: Vec::new(),
    })
}

#[tauri::command]
async fn list_skill_commits(
    workspace: String,
    skill_path: String,
    ref_name: Option<String>,
    limit: Option<u32>,
) -> CommandResult<Vec<CommitSummary>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before listing commits",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;
    let path = skill_path.trim_matches('/');
    provider
        .list_path_commits(&workspace, path, ref_name.as_deref(), limit.unwrap_or(30))
        .await
        .map_err(provider_command_error)
}

#[tauri::command]
async fn list_workspace_pull_requests(
    workspace: String,
    state: Option<String>,
) -> CommandResult<Vec<PullRequestSummary>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before listing PRs",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;
    let state = match state.as_deref() {
        Some("closed") => PullRequestQueryState::Closed,
        Some("all") => PullRequestQueryState::All,
        _ => PullRequestQueryState::Open,
    };
    provider
        .list_pull_requests(&workspace, state)
        .await
        .map_err(provider_command_error)
}

#[tauri::command]
async fn list_workspace_pull_request_files(
    workspace: String,
    number: u64,
) -> CommandResult<Vec<ChangedFile>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before reading PR files",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;
    provider
        .list_pull_request_files(&workspace, number)
        .await
        .map_err(provider_command_error)
}

#[tauri::command]
async fn merge_workspace_pull_request(
    workspace: String,
    number: u64,
    head_ref: String,
    head_repo: Option<String>,
    delete_branch: Option<bool>,
) -> CommandResult<PublishAutoMergeResult> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before merging PRs",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;
    let result = provider
        .merge_pull_request(&workspace, number)
        .await
        .map_err(provider_command_error)?;
    let mut deleted_branch = false;
    let mut error = None;
    let can_delete_head = head_repo
        .as_deref()
        .map(|repo| repo == workspace.full_name())
        .unwrap_or(false);
    if delete_branch.unwrap_or(true)
        && can_delete_head
        && is_skill_library_publish_branch(&head_ref)
    {
        match provider.delete_branch(&workspace, &head_ref).await {
            Ok(()) => deleted_branch = true,
            Err(err) => error = Some(format!("merged, but branch cleanup failed: {err}")),
        }
    }
    Ok(PublishAutoMergeResult {
        merged: result.state == "closed",
        deleted_branch,
        error,
    })
}

#[tauri::command]
async fn close_workspace_pull_request(
    workspace: String,
    number: u64,
    comment: Option<String>,
) -> CommandResult<PullRequestSummary> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before closing PRs",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;
    if let Some(comment) = comment
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        provider
            .add_pull_request_comment(&workspace, number, comment)
            .await
            .map_err(provider_command_error)?;
    }
    provider
        .close_pull_request(&workspace, number)
        .await
        .map_err(provider_command_error)
}

#[tauri::command]
async fn add_workspace_pull_request_comment(
    workspace: String,
    number: u64,
    body: String,
) -> CommandResult<IssueComment> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before commenting on PRs",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;
    let body = body.trim();
    if body.is_empty() {
        return Err(CommandError::coded(
            "empty_comment",
            "comment cannot be empty",
        ));
    }
    provider
        .add_pull_request_comment(&workspace, number, body)
        .await
        .map_err(provider_command_error)
}

#[tauri::command]
async fn list_workspace_events(workspace: String) -> CommandResult<Vec<RepositoryEvent>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before listing activity",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;
    provider
        .list_repository_events(&workspace)
        .await
        .map_err(provider_command_error)
}

#[tauri::command]
async fn list_repository_invitations() -> CommandResult<Vec<RepositoryInvitation>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before listing invitations",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    provider
        .list_user_repository_invitations()
        .await
        .map_err(provider_command_error)
}

#[tauri::command]
async fn accept_repository_invitation(invitation_id: u64) -> CommandResult<()> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before accepting an invitation",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    provider
        .accept_user_repository_invitation(invitation_id)
        .await
        .map_err(provider_command_error)
}

fn selection_from_targets(targets: Vec<String>) -> TargetSelection {
    // No "empty → all tools" fallback: an empty list is a deliberate
    // "download locally, deploy nowhere" choice (all switches off in the UI).
    TargetSelection {
        claude_code: targets.iter().any(|target| target == "claude-code"),
        cursor: targets.iter().any(|target| target == "cursor"),
        codex: targets.iter().any(|target| target == "codex"),
        custom: targets
            .into_iter()
            .filter(|target| !matches!(target.as_str(), "claude-code" | "cursor" | "codex"))
            .collect(),
    }
}

fn parse_permission_role(value: &str) -> CommandResult<PermissionLevel> {
    match value {
        "read" => Ok(PermissionLevel::Read),
        "triage" => Ok(PermissionLevel::Triage),
        "write" => Ok(PermissionLevel::Write),
        "maintain" => Ok(PermissionLevel::Maintain),
        "admin" => Ok(PermissionLevel::Admin),
        other => Err(CommandError::coded(
            "invalid_invitation_role",
            format!("unsupported invitation role `{other}`"),
        )),
    }
}

fn workspace_webhook_registration(
    webhook_url: Option<String>,
    webhook_secret: Option<String>,
    webhook_events: Option<Vec<String>>,
) -> CommandResult<Option<WorkspaceWebhookRegistration>> {
    let events = webhook_events.unwrap_or_default();
    match (webhook_url, webhook_secret) {
        (Some(callback_url), Some(secret)) => Ok(Some(WorkspaceWebhookRegistration {
            callback_url,
            secret,
            events,
        })),
        (None, None) if events.is_empty() => Ok(None),
        (None, _) => Err(CommandError::coded(
            "missing_webhook_url",
            "webhookUrl is required when registering a workspace webhook",
        )),
        (_, None) => Err(CommandError::coded(
            "missing_webhook_secret",
            "webhookSecret is required when registering a workspace webhook",
        )),
    }
}

fn saved_github_token(paths: &AppPaths) -> Option<String> {
    skill_library_core::load_github_credential(&paths.credentials)
        .ok()
        .flatten()
        .map(|github| github.token)
}

// ---------------------------------------------------------------------------
// AI review — provider API key stored in the OS keychain, and a command that
// sends a skill's SKILL.md to the configured LLM for a safety review.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteReviewCommit {
    path: String,
    sha: String,
}

fn review_file_path(skill_id: &str) -> CommandResult<String> {
    let cleaned = skill_id
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if cleaned.is_empty() {
        return Err(CommandError::coded(
            "invalid_skill_id",
            "skill id is required for review sync",
        ));
    }
    Ok(format!(".reviews/{cleaned}.json"))
}

#[tauri::command]
async fn get_skill_content_hash(
    workspace: String,
    skill_path: String,
    ref_name: Option<String>,
) -> CommandResult<String> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&workspace)?;
    let token = saved_github_token(&paths);
    let prepared = skill_library_sync::prepare_skill_for_review(
        &paths,
        &workspace,
        &skill_path,
        ref_name.as_deref(),
        token.as_deref(),
    )
    .await
    .map_err(|err| CommandError::coded("ai_download", err.to_string()))?;

    ai_review::content_hash_for_dir(&prepared.skill_dir).map_err(map_review_error)
}

#[tauri::command]
async fn get_remote_review(workspace: String, skill_id: String) -> CommandResult<Option<String>> {
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths);
    let provider = github_provider(token.as_deref())?;
    let workspace = parse_workspace(&workspace)?;
    let ws_info = provider
        .get_workspace(&workspace)
        .await
        .map_err(provider_command_error)?;
    let path = review_file_path(&skill_id)?;

    match provider
        .read_file(&workspace, &GitRef::Branch(ws_info.default_branch), &path)
        .await
    {
        Ok(blob) => {
            let content = String::from_utf8(blob.bytes)
                .map_err(|err| CommandError::coded("invalid_review_cache", err.to_string()))?;
            Ok(Some(content))
        }
        Err(skill_library_provider::ProviderError::NotFound { .. }) => Ok(None),
        Err(err) => Err(provider_command_error(err)),
    }
}

#[tauri::command]
async fn commit_review_to_repo(
    workspace: String,
    skill_id: String,
    review_json: String,
) -> CommandResult<RemoteReviewCommit> {
    serde_json::from_str::<serde_json::Value>(&review_json)
        .map_err(|err| CommandError::coded("invalid_review_json", err.to_string()))?;

    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths).ok_or_else(|| {
        CommandError::coded(
            "missing_github_token",
            "log in with GitHub before syncing a review",
        )
    })?;
    let provider = GitHubProvider::new(token).map_err(provider_command_error)?;
    let workspace = parse_workspace(&workspace)?;
    let ws_info = provider
        .get_workspace(&workspace)
        .await
        .map_err(provider_command_error)?;
    let path = review_file_path(&skill_id)?;
    let uploaded = provider
        .put_file_content(
            &workspace,
            &ws_info.default_branch,
            &path,
            review_json.as_bytes(),
            &format!("Update AI review for {skill_id}"),
        )
        .await
        .map_err(provider_command_error)?;

    Ok(RemoteReviewCommit {
        path: uploaded.path,
        sha: uploaded.sha,
    })
}

/// Batch-fetch the remote review JSON for many skills in a single request. The
/// returned vec is index-aligned with `skill_ids`; entries are `None` when the
/// skill has no `.reviews/{id}.json` yet. Used to warm the local review cache on
/// workspace load (and drive the "reviewed safe" badge) without one round-trip
/// per skill.
#[tauri::command]
async fn get_remote_reviews_batch(
    workspace: String,
    skill_ids: Vec<String>,
) -> CommandResult<Vec<Option<String>>> {
    if skill_ids.is_empty() {
        return Ok(Vec::new());
    }
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let token = saved_github_token(&paths);
    let provider = github_provider(token.as_deref())?;
    let workspace = parse_workspace(&workspace)?;
    let ws_info = provider
        .get_workspace(&workspace)
        .await
        .map_err(provider_command_error)?;
    let review_paths: Vec<String> = skill_ids
        .iter()
        .map(|id| review_file_path(id).unwrap_or_else(|_| ".reviews/__invalid__.json".to_string()))
        .collect();
    provider
        .batch_fetch_text_files(&workspace, &ws_info.default_branch, &review_paths)
        .await
        .map_err(provider_command_error)
}

#[tauri::command]
fn save_ai_key(key: String) -> CommandResult<()> {
    skill_library_core::write_ai_key(&key)
        .map_err(|err| CommandError::coded("ai_key_save", err.to_string()))
}

#[tauri::command]
fn delete_ai_key() -> CommandResult<()> {
    skill_library_core::delete_ai_key()
        .map_err(|err| CommandError::coded("ai_key_delete", err.to_string()))
}

/// Returns whether an AI API key is currently stored (never returns the key).
#[tauri::command]
fn has_ai_key() -> CommandResult<bool> {
    Ok(skill_library_core::read_ai_key()
        .map_err(|err| CommandError::coded("ai_key_read", err.to_string()))?
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false))
}

#[tauri::command]
async fn review_skill(request: ai_review::ReviewRequest) -> CommandResult<ai_review::ReviewResult> {
    if request.provider.trim().is_empty() || request.provider == "none" {
        return Err(CommandError::coded(
            "ai_not_configured",
            "configure an AI provider in Settings first",
        ));
    }
    if request.base_url.trim().is_empty() {
        return Err(CommandError::coded(
            "ai_not_configured",
            "set the provider base URL in Settings",
        ));
    }
    let key = skill_library_core::read_ai_key()
        .map_err(|err| CommandError::coded("ai_key_read", err.to_string()))?
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| CommandError::coded("ai_missing_key", "add an AI API key in Settings"))?;

    // Download (or reuse a cached copy of) the entire skill source so the whole
    // bundle is reviewed, not just SKILL.md. Token is optional (public skills
    // can be reviewed anonymously).
    let paths = AppPaths::resolve()?;
    skill_library_sync::ensure_local_state(&paths)?;
    let workspace = parse_workspace(&request.workspace)?;
    let token = saved_github_token(&paths);
    let prepared = skill_library_sync::prepare_skill_for_review(
        &paths,
        &workspace,
        &request.skill_path,
        request.ref_name.as_deref(),
        token.as_deref(),
    )
    .await
    .map_err(|err| CommandError::coded("ai_download", err.to_string()))?;

    ai_review::review_skill(&request, &prepared.skill_dir, &key)
        .await
        .map_err(|err| match err {
            ai_review::ReviewError::NotConfigured => {
                CommandError::coded("ai_not_configured", err.to_string())
            }
            ai_review::ReviewError::UnsupportedProvider(_) => {
                CommandError::coded("ai_unsupported_provider", err.to_string())
            }
            ai_review::ReviewError::Io(_) => CommandError::coded("ai_download", err.to_string()),
            ai_review::ReviewError::Network(_) => {
                CommandError::coded("ai_network", err.to_string())
            }
            ai_review::ReviewError::Provider { .. } => {
                CommandError::coded("ai_provider_error", err.to_string())
            }
            ai_review::ReviewError::Parse(_) => CommandError::coded("ai_parse", err.to_string()),
        })
}

/// Review an already-installed ("My Skills") skill straight from its local copy
/// on disk — no GitHub download. The AI provider config comes from the caller
/// (the same Settings the discover-page review uses); the API key is read from
/// the keychain. The verdict + findings are cached back into SQLite, stamped
/// with the skill's current content hash so a later content change marks it
/// stale.
#[tauri::command]
async fn review_local_skill(
    app: tauri::AppHandle,
    skill_id: String,
    provider: String,
    base_url: String,
    model: String,
    language: Option<String>,
) -> CommandResult<ai_review::ReviewResult> {
    if provider.trim().is_empty() || provider == "none" {
        return Err(CommandError::coded(
            "ai_not_configured",
            "configure an AI provider in Settings first",
        ));
    }
    if base_url.trim().is_empty() {
        return Err(CommandError::coded(
            "ai_not_configured",
            "set the provider base URL in Settings",
        ));
    }
    let key = skill_library_core::read_ai_key()
        .map_err(|err| CommandError::coded("ai_key_read", err.to_string()))?
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| CommandError::coded("ai_missing_key", "add an AI API key in Settings"))?;

    // Look up the skill's on-disk location (lock released before the await).
    let skill = {
        let database = app.state::<Mutex<db::Database>>();
        let db_guard = database.lock().unwrap();
        db_guard
            .get_skill(&skill_id)
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?
            .ok_or_else(|| {
                CommandError::coded(
                    "skill_not_found",
                    format!("skill '{skill_id}' not in registry"),
                )
            })?
    };

    let local_path = PathBuf::from(&skill.local_path);
    if !local_path.is_dir() {
        return Err(CommandError::coded(
            "skill_files_missing",
            format!("skill files not found at {}", skill.local_path),
        ));
    }

    // Pull declared permissions from the manifest for extra prompt context.
    let permissions = skill_library_manifest::parse_skill_dir(&local_path)
        .ok()
        .and_then(|p| p.manifest)
        .map(|m| m.permissions)
        .unwrap_or_default();

    let request = ai_review::ReviewRequest {
        provider,
        base_url,
        model,
        language,
        // workspace/skill_path/ref_name are only used by the download path; the
        // local reviewer walks `local_path` directly, so they stay empty.
        workspace: String::new(),
        skill_path: String::new(),
        ref_name: None,
        skill_name: if skill.name.is_empty() {
            skill_id.clone()
        } else {
            skill.name.clone()
        },
        permissions,
    };

    let result = ai_review::review_skill(&request, &local_path, &key)
        .await
        .map_err(map_review_error)?;

    // Cache the verdict, stamped with the current content hash for staleness.
    let current_hash = db::compute_dir_hash(&local_path);
    let findings_json = serde_json::to_string(&result.findings).unwrap_or_default();
    {
        let database = app.state::<Mutex<db::Database>>();
        let db_guard = database.lock().unwrap();
        db_guard
            .save_review(
                &skill_id,
                &result.verdict,
                &result.summary,
                &findings_json,
                &current_hash,
            )
            .map_err(|e| CommandError::coded("db_error", e.to_string()))?;
    }

    Ok(result)
}

/// Shared mapping of an `ai_review::ReviewError` to a coded command error.
fn map_review_error(err: ai_review::ReviewError) -> CommandError {
    match err {
        ai_review::ReviewError::NotConfigured => {
            CommandError::coded("ai_not_configured", err.to_string())
        }
        ai_review::ReviewError::UnsupportedProvider(_) => {
            CommandError::coded("ai_unsupported_provider", err.to_string())
        }
        ai_review::ReviewError::Io(_) => CommandError::coded("ai_download", err.to_string()),
        ai_review::ReviewError::Network(_) => CommandError::coded("ai_network", err.to_string()),
        ai_review::ReviewError::Provider { .. } => {
            CommandError::coded("ai_provider_error", err.to_string())
        }
        ai_review::ReviewError::Parse(_) => CommandError::coded("ai_parse", err.to_string()),
    }
}

fn github_provider(token: Option<&str>) -> CommandResult<GitHubProvider> {
    match token {
        Some(token) if !token.trim().is_empty() => {
            GitHubProvider::new(token.to_owned()).map_err(provider_command_error)
        }
        _ => GitHubProvider::anonymous("https://api.github.com").map_err(provider_command_error),
    }
}

fn provider_command_error(err: skill_library_provider::ProviderError) -> CommandError {
    CommandError::coded("provider_error", err.to_string())
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn init_tracing() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,skill-library=debug,skill_library_github=debug,skill-library-github=debug",
        )
    });

    let log_dir = AppPaths::resolve()
        .map(|paths| paths.logs.clone())
        .unwrap_or_else(|_| std::env::temp_dir().join("skill-library").join("logs"));
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "skill-library.log");

    let stderr_layer = fmt::layer().with_target(true).with_writer(std::io::stderr);
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(file_appender);

    if let Err(err) = tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init()
    {
        eprintln!("tracing init failed: {err}");
    }
}

fn resolve_github_client_id(client_id: Option<String>) -> CommandResult<String> {
    // Three-tier resolution so end users never need to know about client IDs:
    //   1. explicit override from the UI (used in development / advanced flows)
    //   2. runtime env var (lets ops swap the OAuth app without rebuilding)
    //   3. compile-time default baked in via `SKILL_LIBRARY_GITHUB_CLIENT_ID`
    let baked = option_env!("SKILL_LIBRARY_GITHUB_CLIENT_ID").map(str::to_owned);
    let value = client_id
        .or_else(|| std::env::var("GITHUB_CLIENT_ID").ok())
        .or(baked)
        .unwrap_or_default();
    let trimmed = value.trim().to_owned();
    if trimmed.is_empty() {
        return Err(CommandError::coded(
            "missing_github_client_id",
            "Skill Library is not configured for GitHub sign-in. Build with SKILL_LIBRARY_GITHUB_CLIENT_ID set, or export GITHUB_CLIENT_ID before launching.",
        ));
    }
    Ok(trimmed)
}

fn default_runtime_targets() -> Vec<String> {
    vec!["claude-code".to_owned(), "codex".to_owned()]
}

fn local_agent_root_specs(home: &Path) -> Vec<LocalAgentRoot> {
    vec![
        LocalAgentRoot {
            id: "cursor-agents".to_owned(),
            label: "Cursor Agents".to_owned(),
            kind: "cursor".to_owned(),
            path: home.join(".cursor").join("agents"),
            exists: false,
            entries: Vec::new(),
        },
        LocalAgentRoot {
            id: "cursor-skills".to_owned(),
            label: "Cursor Skills".to_owned(),
            kind: "cursor".to_owned(),
            path: home.join(".cursor").join("skills"),
            exists: false,
            entries: Vec::new(),
        },
        LocalAgentRoot {
            id: "claude-agents".to_owned(),
            label: "Claude Agents".to_owned(),
            kind: "claude".to_owned(),
            path: home.join(".claude").join("agents"),
            exists: false,
            entries: Vec::new(),
        },
        LocalAgentRoot {
            id: "claude-skills".to_owned(),
            label: "Claude Skills".to_owned(),
            kind: "claude".to_owned(),
            path: home.join(".claude").join("skills"),
            exists: false,
            entries: Vec::new(),
        },
        LocalAgentRoot {
            id: "shared-agents-skills".to_owned(),
            label: "Shared Agent Skills".to_owned(),
            kind: "shared".to_owned(),
            path: home.join(".agents").join("skills"),
            exists: false,
            entries: Vec::new(),
        },
        LocalAgentRoot {
            id: "codex-skills".to_owned(),
            label: "Codex Skills".to_owned(),
            kind: "codex".to_owned(),
            path: home.join(".codex").join("skills"),
            exists: false,
            entries: Vec::new(),
        },
    ]
}

fn scan_local_agent_root(mut root: LocalAgentRoot) -> LocalAgentRoot {
    root.exists = root.path.is_dir();
    if !root.exists {
        return root;
    }
    let Ok(entries) = fs::read_dir(&root.path) else {
        return root;
    };
    let mut scanned = entries
        .flatten()
        .filter_map(|entry| local_agent_entry(entry.path()))
        .collect::<Vec<_>>();
    scanned.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
    root.entries = scanned;
    root
}

fn local_agent_entry(path: PathBuf) -> Option<LocalAgentEntry> {
    if !path.is_dir() {
        return None;
    }
    let id = path.file_name()?.to_string_lossy().to_string();
    if id.starts_with('.') {
        return None;
    }
    let manifest_path = path.join("manifest.yaml");
    let skill_md_path = path.join("SKILL.md");
    let install_metadata_path = path.join(".skill-library-install.json");
    let manifest = skill_library_manifest::parse_skill_dir(&path)
        .ok()
        .and_then(|parsed| parsed.manifest);
    Some(LocalAgentEntry {
        name: manifest
            .as_ref()
            .map(|value| value.name.clone())
            .unwrap_or_else(|| humanize_agent_dir_name(&id)),
        version: manifest.as_ref().map(|value| value.version.clone()),
        description: manifest.as_ref().map(|value| value.description.clone()),
        has_manifest: manifest_path.exists(),
        has_skill_md: skill_md_path.exists(),
        managed: install_metadata_path.exists(),
        id,
        path,
    })
}

fn humanize_agent_dir_name(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn run() {
    // Load .env file(s) so GITHUB_CLIENT_ID and other vars are available at runtime.
    // Silently ignore if no .env exists (e.g. production builds with baked-in values).
    let _ = dotenvy::dotenv();

    init_tracing();

    // Initialize SQLite database
    let paths = AppPaths::resolve().expect("failed to resolve app paths");
    let db_path = paths.home.join("db.sqlite");
    let database = db::Database::open(&db_path).expect("failed to open database");
    // A tarball download can't resume across restarts, so any row left in the
    // 'downloading' state from a previous session (the app was closed mid-fetch)
    // is reconciled to 'error' here — surfacing it as interrupted + retryable
    // rather than a progress bar stuck forever.
    if let Ok(count) = database.reconcile_interrupted_downloads() {
        if count > 0 {
            tracing::info!(
                count,
                "reconciled interrupted downloads from previous session"
            );
        }
    }

    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("appicon", |_ctx, request, responder| {
            std::thread::spawn(move || {
                responder.respond(app_icons::handle_icon_request(request));
            });
        })
        .manage(DeepLinkState::default())
        .manage(RegistryCache::default())
        .manage(Mutex::new(database))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let urls = args
                .into_iter()
                .filter_map(|arg| arg.parse::<Url>().ok())
                .collect::<Vec<_>>();
            for url in urls {
                register_deep_link(app, url);
            }
        }))
        .setup(|app| {
            let handle = app.handle().clone();
            let window = app.get_webview_window("main");
            if let Some(window) = window {
                let _ = window.set_title("Skill Library");
            }
            if let Some(deep_link) =
                handle.try_state::<tauri_plugin_deep_link::DeepLink<tauri::Wry>>()
            {
                if let Ok(Some(urls)) = deep_link.get_current() {
                    for url in urls {
                        register_deep_link(&handle, url);
                    }
                }
                let _ = deep_link.on_open_url({
                    let app = handle.clone();
                    move |event| {
                        for url in event.urls() {
                            register_deep_link(&app, url);
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_init,
            add_workspace,
            get_deep_link_state,
            check_workspace_head,
            diff_workspace_since,
            compare_skill_versions,
            get_auth_status,
            start_github_device_flow,
            poll_github_device_flow,
            login_github_token,
            logout_github,
            get_skill_detail,
            get_workspace_detail,
            invite_github_collaborator,
            list_workspace_members,
            export_diagnostics,
            open_logs_folder,
            list_github_workspaces,
            search_skills_registry,
            save_ai_key,
            delete_ai_key,
            has_ai_key,
            get_skill_content_hash,
            get_remote_review,
            get_remote_reviews_batch,
            commit_review_to_repo,
            review_skill,
            list_workspaces,
            scan_workspace,
            scan_github_workspace,
            scan_github_workspace_streaming,
            parse_skill,
            read_subscriptions,
            subscribe_workspace_skill,
            sync_now,
            install_skill,
            remove_skill,
            list_installed_targets,
            list_local_agent_roots,
            preview_publish,
            preview_publish_from_workspace,
            publish_skill_to_workspace,
            publish_workspace_skill_update,
            list_workspace_pull_requests,
            list_workspace_pull_request_files,
            merge_workspace_pull_request,
            close_workspace_pull_request,
            add_workspace_pull_request_comment,
            list_workspace_events,
            list_repository_invitations,
            accept_repository_invitation,
            list_skill_commits,
            list_skill_files,
            list_workspace_branches,
            read_skill_file,
            cache_skill_package,
            list_skill_discussions,
            get_discussion_by_number,
            get_discussion_comments,
            add_discussion_comment,
            toggle_discussion_reaction,
            remove_discussion_reaction,
            create_skill_discussion,
            db_list_runtimes,
            db_list_skills,
            db_enable_skill,
            db_disable_skill,
            db_check_project_deployments,
            db_add_project_deployments,
            db_scan_unmanaged,
            db_import_skill,
            db_cache_stats,
            db_clear_cache,
            db_cache_get,
            db_cache_get_many,
            db_cache_put,
            db_cache_delete,
            db_cache_delete_prefix,
            remote_cache_put_file,
            remote_cache_get_file,
            remote_cache_delete_skill,
            remote_cache_delete_workspace,
            remote_cache_stats,
            db_check_modifications,
            db_unmanage_skill,
            download_skill_async,
            review_local_skill,
            open_data_dir,
            open_local_path,
            list_path_openers,
            db_set_project_deployment_enabled,
            db_delete_project_deployment
        ])
        .run(tauri::generate_context!())
        .expect("error while running Skill Library");
}

#[cfg(test)]
mod tests {
    use super::{
        default_runtime_targets, humanize_agent_dir_name, local_agent_root_specs,
        redact_sensitive_text, CommandError,
    };
    use std::path::Path;

    #[test]
    fn command_error_serializes_as_structured_object() {
        let value = serde_json::to_value(CommandError::coded(
            "missing_github_token",
            "GitHub token is required",
        ))
        .unwrap();

        assert_eq!(value["code"], "missing_github_token");
        assert_eq!(value["message"], "GitHub token is required");
    }

    #[test]
    fn diagnostics_redaction_removes_token_like_values() {
        let redacted = redact_sensitive_text(
            "ghp_abcdefghijklmnopqrstuvwxyz123456 github_pat_11_secret GITHUB_TOKEN",
        );

        assert!(!redacted.contains("ghp_"));
        assert!(!redacted.contains("github_pat_"));
        assert!(!redacted.contains("GITHUB_TOKEN"));
        assert_eq!(redacted.matches("[REDACTED]").count(), 3);
    }

    #[test]
    fn installed_targets_default_to_supported_agent_runtimes() {
        assert_eq!(
            default_runtime_targets(),
            vec!["claude-code".to_owned(), "codex".to_owned()]
        );
    }

    #[test]
    fn local_agent_roots_include_ide_agents_and_shared_skills() {
        let roots = local_agent_root_specs(Path::new("/home/demo"));
        let paths = roots
            .iter()
            .map(|root| root.path.clone())
            .collect::<Vec<_>>();

        // Use Path::ends_with (component-wise match) so this passes regardless
        // of the platform's path separator (e.g. backslashes on Windows).
        assert!(paths.iter().any(|path| path.ends_with(".cursor/agents")));
        assert!(paths.iter().any(|path| path.ends_with(".claude/agents")));
        assert!(paths.iter().any(|path| path.ends_with(".agents/skills")));
        assert!(paths.iter().any(|path| path.ends_with(".codex/skills")));
    }

    #[test]
    fn local_agent_directory_names_are_humanized() {
        assert_eq!(
            humanize_agent_dir_name("code-reviewer_agent"),
            "Code Reviewer Agent"
        );
    }

    #[test]
    fn path_opener_exposes_protocol_icon_urls() {
        let opener = super::path_opener(&super::app_icons::PATH_OPENER_CANDIDATES[0]);
        let urls = opener
            .icon_urls
            .expect("path opener should include icon urls");

        assert_eq!(opener.id, "vscode");
        assert_eq!(
            opener.icon_url.as_deref(),
            Some("appicon://localhost/vscode?size=default&scale=2&v=2")
        );
        assert_eq!(
            urls.small,
            "appicon://localhost/vscode?size=small&scale=2&v=2"
        );
        assert_eq!(
            urls.default_size,
            "appicon://localhost/vscode?size=default&scale=2&v=2"
        );
        assert_eq!(
            urls.large,
            "appicon://localhost/vscode?size=large&scale=2&v=2"
        );
    }
}
