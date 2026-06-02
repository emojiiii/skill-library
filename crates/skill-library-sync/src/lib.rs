use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use skill_library_core::{AppPaths, RiskLevel, UpdatePolicy, WorkspaceRef};
use skill_library_installer::{InstallOptions, InstallReport, TargetRoot};
use skill_library_manifest::{effective_risk, SkillAsset};
use skill_library_provider::{PageOpts, Provider, ProviderError, WebhookConfig, Workspace};
use skill_library_provider_github::{
    scan::{
        read_skill_detail, scan_skill_assets_streaming, scan_workspace_detail,
        scan_workspace_skills, SkillDetailScan, WorkspaceDetailScan,
    },
    GitHubProvider,
};
use std::fs;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, SyncError>;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("semver error: {0}")]
    Semver(#[from] semver::Error),
    #[error("manifest error: {0}")]
    Manifest(#[from] skill_library_manifest::ManifestError),
    #[error("installer error: {0}")]
    Installer(#[from] skill_library_installer::InstallerError),
    #[error("invalid skill source: {0}")]
    InvalidSource(String),
    #[error("subscription not found: {0}")]
    NotFound(String),
    #[error("remote resource not found: {0}")]
    RemoteNotFound(String),
    #[error(
        "risk confirmation required for {asset_id}: {risk_level} risk permissions [{permissions}]"
    )]
    RiskConfirmationRequired {
        asset_id: String,
        risk_level: RiskLevel,
        permissions: String,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubscriptionsFile {
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subscription {
    pub workspace: WorkspaceRef,
    pub asset_id: String,
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub update: UpdatePolicy,
    #[serde(default)]
    pub targets: TargetSelection,
    #[serde(default)]
    pub subscribed_at: Option<DateTime<Utc>>,
}

fn default_channel() -> String {
    "stable".to_owned()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetSelection {
    #[serde(default)]
    pub claude_code: bool,
    #[serde(default)]
    pub cursor: bool,
    #[serde(default)]
    pub codex: bool,
    #[serde(default)]
    pub custom: Vec<String>,
}

impl TargetSelection {
    pub fn all_default() -> Self {
        Self {
            claude_code: true,
            cursor: false,
            codex: true,
            custom: Vec::new(),
        }
    }

    pub fn enabled_targets(&self) -> Vec<String> {
        let mut targets = Vec::new();
        if self.claude_code {
            targets.push("claude-code".to_owned());
        }
        if self.cursor {
            targets.push("cursor".to_owned());
        }
        if self.codex {
            targets.push("codex".to_owned());
        }
        targets.extend(self.custom.clone());
        targets
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockFile {
    #[serde(default)]
    pub assets: Vec<LockedAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedAsset {
    pub workspace: WorkspaceRef,
    pub asset_id: String,
    pub version: String,
    pub ref_name: String,
    pub pinned: bool,
    pub installed_at: DateTime<Utc>,
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UpdateDecision {
    Install { version: String },
    Keep { reason: String },
    Manual { version: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteWorkspaceSkills {
    pub workspace: Workspace,
    pub skills: Vec<SkillAsset>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncOptions {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub target_roots: Vec<TargetRoot>,
    #[serde(default)]
    pub source_override: Option<PathBuf>,
    #[serde(default)]
    pub allow_risky: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub synced_at: DateTime<Utc>,
    pub items: Vec<SyncItemReport>,
}

impl SyncReport {
    pub fn risk_confirmation_requests(&self) -> Vec<RiskConfirmationRequest> {
        self.items
            .iter()
            .filter_map(SyncItemReport::risk_confirmation_request)
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncItemReport {
    pub workspace: WorkspaceRef,
    pub asset_id: String,
    pub ref_name: Option<String>,
    pub version: Option<String>,
    pub decision: UpdateDecision,
    pub source_path: Option<PathBuf>,
    pub source_hash: Option<String>,
    pub install: Option<InstallReport>,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_confirmation: Option<RiskConfirmationDetail>,
}

impl SyncItemReport {
    pub fn risk_confirmation_request(&self) -> Option<RiskConfirmationRequest> {
        self.risk_confirmation
            .clone()
            .map(|detail| RiskConfirmationRequest {
                workspace: self.workspace.clone(),
                asset_id: self.asset_id.clone(),
                risk_level: detail.risk_level,
                permissions: detail.permissions,
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiskConfirmationDetail {
    pub risk_level: RiskLevel,
    pub permissions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiskConfirmationRequest {
    pub workspace: WorkspaceRef,
    pub asset_id: String,
    pub risk_level: RiskLevel,
    pub permissions: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RollbackOptions {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub target_roots: Vec<TargetRoot>,
    #[serde(default)]
    pub source_override: Option<PathBuf>,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub allow_risky: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackReport {
    pub workspace: WorkspaceRef,
    pub asset_id: String,
    pub version: String,
    pub ref_name: String,
    pub source_path: PathBuf,
    pub source_hash: Option<String>,
    pub install: InstallReport,
    pub lock: LockFile,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspacesFile {
    #[serde(default)]
    pub workspaces: Vec<StoredWorkspace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredWorkspace {
    pub provider: String,
    pub owner: String,
    pub repo: String,
    pub full_name: String,
    pub default_branch: String,
    pub visibility: String,
    pub permission: String,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook: Option<StoredWebhook>,
    #[serde(default = "default_registered_at")]
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredWebhook {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub callback_url: Option<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default = "default_registered_at")]
    pub registered_at: DateTime<Utc>,
}

fn default_registered_at() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceWebhookRegistration {
    pub callback_url: String,
    pub secret: String,
    #[serde(default)]
    pub events: Vec<String>,
}

impl WorkspaceWebhookRegistration {
    pub fn push(callback_url: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            callback_url: callback_url.into(),
            secret: secret.into(),
            events: vec!["push".to_owned()],
        }
    }
}

impl StoredWorkspace {
    pub fn reference(&self) -> WorkspaceRef {
        WorkspaceRef {
            provider: self.provider.clone(),
            owner: self.owner.clone(),
            repo: self.repo.clone(),
        }
    }
}

impl From<Workspace> for StoredWorkspace {
    fn from(workspace: Workspace) -> Self {
        Self {
            provider: workspace.provider,
            owner: workspace.owner,
            repo: workspace.repo,
            full_name: workspace.full_name,
            default_branch: workspace.default_branch,
            visibility: workspace.visibility,
            permission: format!("{:?}", workspace.permission).to_lowercase(),
            html_url: workspace.html_url,
            webhook: None,
            added_at: Utc::now(),
        }
    }
}

pub fn ensure_local_state(paths: &AppPaths) -> Result<()> {
    paths.ensure().map_err(|err| match err {
        skill_library_core::SkillLibraryError::Io(err) => SyncError::Io(err),
        other => SyncError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            other.to_string(),
        )),
    })?;
    if !paths.subscriptions.exists() {
        write_subscriptions(&paths.subscriptions, &SubscriptionsFile::default())?;
    }
    if !paths.workspace_registry.exists() {
        write_workspaces(&paths.workspace_registry, &WorkspacesFile::default())?;
    }
    Ok(())
}

pub fn read_subscriptions(path: impl AsRef<Path>) -> Result<SubscriptionsFile> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(SubscriptionsFile::default());
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&raw)?)
}

pub fn write_subscriptions(path: impl AsRef<Path>, value: &SubscriptionsFile) -> Result<()> {
    atomic_write(path.as_ref(), serde_yaml::to_string(value)?.as_bytes())
}

pub fn read_workspaces(path: impl AsRef<Path>) -> Result<WorkspacesFile> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(WorkspacesFile::default());
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&raw)?)
}

pub fn write_workspaces(path: impl AsRef<Path>, value: &WorkspacesFile) -> Result<()> {
    atomic_write(path.as_ref(), serde_yaml::to_string(value)?.as_bytes())
}

pub fn upsert_workspace(
    path: impl AsRef<Path>,
    workspace: StoredWorkspace,
) -> Result<WorkspacesFile> {
    let path = path.as_ref();
    let mut file = read_workspaces(path)?;
    file.workspaces.retain(|existing| {
        existing.full_name != workspace.full_name || existing.provider != workspace.provider
    });
    file.workspaces.push(workspace);
    file.workspaces.sort_by(|a, b| {
        a.full_name
            .cmp(&b.full_name)
            .then_with(|| a.provider.cmp(&b.provider))
    });
    write_workspaces(path, &file)?;
    Ok(file)
}

pub fn subscribe(
    path: impl AsRef<Path>,
    mut subscription: Subscription,
) -> Result<SubscriptionsFile> {
    let path = path.as_ref();
    let mut file = read_subscriptions(path)?;
    subscription.subscribed_at.get_or_insert_with(Utc::now);
    // No "empty targets → all tools" fallback: an empty selection is a
    // deliberate "download locally, deploy nowhere" choice and must be honored.
    file.subscriptions.retain(|existing| {
        !(existing.workspace == subscription.workspace
            && existing.asset_id == subscription.asset_id)
    });
    file.subscriptions.push(subscription);
    file.subscriptions.sort_by(|a, b| {
        a.workspace
            .full_name()
            .cmp(&b.workspace.full_name())
            .then_with(|| a.asset_id.cmp(&b.asset_id))
    });
    write_subscriptions(path, &file)?;
    Ok(file)
}

pub fn unsubscribe(
    path: impl AsRef<Path>,
    workspace: &WorkspaceRef,
    asset_id: &str,
) -> Result<SubscriptionsFile> {
    let path = path.as_ref();
    let mut file = read_subscriptions(path)?;
    let before = file.subscriptions.len();
    file.subscriptions
        .retain(|existing| !(existing.workspace == *workspace && existing.asset_id == asset_id));
    if before == file.subscriptions.len() {
        return Err(SyncError::NotFound(format!(
            "{}:{}",
            workspace.full_name(),
            asset_id
        )));
    }
    write_subscriptions(path, &file)?;
    Ok(file)
}

pub fn read_lockfile(path: impl AsRef<Path>) -> Result<LockFile> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(LockFile::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub fn write_lockfile(path: impl AsRef<Path>, lock: &LockFile) -> Result<()> {
    atomic_write(path.as_ref(), serde_json::to_vec_pretty(lock)?.as_slice())
}

pub fn upsert_locked_asset(path: impl AsRef<Path>, asset: LockedAsset) -> Result<LockFile> {
    let path = path.as_ref();
    let mut lock = read_lockfile(path)?;
    lock.assets.retain(|existing| {
        !(existing.workspace == asset.workspace && existing.asset_id == asset.asset_id)
    });
    lock.assets.push(asset);
    lock.assets.sort_by(|a, b| {
        a.workspace
            .full_name()
            .cmp(&b.workspace.full_name())
            .then_with(|| a.asset_id.cmp(&b.asset_id))
    });
    write_lockfile(path, &lock)?;
    Ok(lock)
}

pub fn decide_update(
    policy: &UpdatePolicy,
    current: Option<&str>,
    latest: &str,
    pinned: bool,
) -> Result<UpdateDecision> {
    if pinned || matches!(policy, UpdatePolicy::Pin) {
        return Ok(UpdateDecision::Keep {
            reason: "pinned".to_owned(),
        });
    }
    let Some(current) = current else {
        return Ok(UpdateDecision::Install {
            version: latest.to_owned(),
        });
    };
    if current == latest {
        return Ok(UpdateDecision::Keep {
            reason: "already current".to_owned(),
        });
    }
    let current_version = Version::parse(current.trim_start_matches('v'))?;
    let latest_version = Version::parse(latest.trim_start_matches('v'))?;
    if latest_version <= current_version {
        return Ok(UpdateDecision::Keep {
            reason: "latest is not newer".to_owned(),
        });
    }
    match policy {
        UpdatePolicy::AutoPatch
            if current_version.major == latest_version.major
                && current_version.minor == latest_version.minor =>
        {
            Ok(UpdateDecision::Install {
                version: latest.to_owned(),
            })
        }
        UpdatePolicy::AutoMinor if current_version.major == latest_version.major => {
            Ok(UpdateDecision::Install {
                version: latest.to_owned(),
            })
        }
        UpdatePolicy::Manual => Ok(UpdateDecision::Manual {
            version: latest.to_owned(),
        }),
        _ => Ok(UpdateDecision::Keep {
            reason: "policy does not allow automatic update".to_owned(),
        }),
    }
}

pub fn workspace_lock_path(paths: &AppPaths, workspace: &WorkspaceRef) -> PathBuf {
    paths
        .workspaces
        .join(workspace.storage_key())
        .join("lock.json")
}

pub async fn scan_github_workspace_skills(
    workspace: &WorkspaceRef,
    token: Option<&str>,
) -> Result<RemoteWorkspaceSkills> {
    if workspace.provider != "github" {
        return Err(SyncError::NotFound(format!(
            "provider {} is not supported",
            workspace.provider
        )));
    }
    let provider = match token {
        Some(token) if !token.trim().is_empty() => {
            GitHubProvider::new(token.to_owned()).map_err(|err| {
                SyncError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    err.to_string(),
                ))
            })?
        }
        _ => GitHubProvider::anonymous("https://api.github.com").map_err(|err| {
            SyncError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                err.to_string(),
            ))
        })?,
    };
    let scan = scan_workspace_skills(&provider, workspace)
        .await
        .map_err(sync_provider_error)?;
    Ok(RemoteWorkspaceSkills {
        workspace: scan.workspace,
        skills: scan.skills,
    })
}

/// Streaming variant: calls `on_batch` after each batch of skills is parsed,
/// allowing the caller to emit incremental progress events.
pub async fn scan_github_workspace_skills_streaming(
    workspace: &WorkspaceRef,
    token: Option<&str>,
    on_batch: impl FnMut(&[SkillAsset]),
) -> Result<RemoteWorkspaceSkills> {
    if workspace.provider != "github" {
        return Err(SyncError::NotFound(format!(
            "provider {} is not supported",
            workspace.provider
        )));
    }
    let provider = github_provider(workspace, token)?;
    let ws = provider
        .get_workspace(workspace)
        .await
        .map_err(sync_provider_error)?;
    let branch = skill_library_provider::GitRef::Branch(ws.default_branch.clone());
    let skills = scan_skill_assets_streaming(&provider, workspace, &branch, on_batch)
        .await
        .map_err(sync_provider_error)?;
    Ok(RemoteWorkspaceSkills {
        workspace: ws,
        skills,
    })
}

pub async fn scan_github_workspace_detail(
    workspace: &WorkspaceRef,
    token: Option<&str>,
) -> Result<WorkspaceDetailScan> {
    let provider = github_provider(workspace, token)?;
    scan_workspace_detail(&provider, workspace)
        .await
        .map_err(sync_provider_error)
}

pub async fn read_github_skill_detail(
    workspace: &WorkspaceRef,
    skill_path: &str,
    ref_name: Option<&str>,
    token: Option<&str>,
) -> Result<SkillDetailScan> {
    let provider = github_provider(workspace, token)?;
    read_skill_detail(&provider, workspace, skill_path, ref_name)
        .await
        .map_err(sync_provider_error)
}

pub async fn add_github_workspace(
    registry_path: impl AsRef<Path>,
    workspace: &WorkspaceRef,
    token: Option<&str>,
) -> Result<StoredWorkspace> {
    add_github_workspace_with_webhook(registry_path, workspace, token, None).await
}

pub async fn add_github_workspace_with_webhook(
    registry_path: impl AsRef<Path>,
    workspace: &WorkspaceRef,
    token: Option<&str>,
    webhook: Option<WorkspaceWebhookRegistration>,
) -> Result<StoredWorkspace> {
    let scan = scan_github_workspace_skills(workspace, token).await?;
    let mut stored = StoredWorkspace::from(scan.workspace);
    if let Some(webhook) = webhook {
        let provider = github_provider(workspace, token)?;
        let events = webhook_events(webhook.events);
        let handle = provider
            .register_webhook(
                workspace,
                WebhookConfig {
                    events: events.clone(),
                    callback_url: webhook.callback_url,
                    secret: webhook.secret,
                },
            )
            .await
            .map_err(sync_provider_error)?;
        stored.webhook = Some(StoredWebhook {
            id: handle.id,
            callback_url: handle.url,
            events,
            registered_at: Utc::now(),
        });
    }
    upsert_workspace(registry_path, stored.clone())?;
    Ok(stored)
}

pub async fn sync_subscriptions(paths: &AppPaths, options: SyncOptions) -> Result<SyncReport> {
    ensure_local_state(paths)?;
    let subscriptions = read_subscriptions(&paths.subscriptions)?;
    let token = options.token.clone();
    let mut items = Vec::new();

    for subscription in subscriptions.subscriptions {
        let report = match sync_subscription(paths, &subscription, &options, token.as_deref()).await
        {
            Ok(report) => report,
            Err(err) => failed_sync_item_report(&subscription, err),
        };
        items.push(report);
    }

    Ok(SyncReport {
        synced_at: Utc::now(),
        items,
    })
}

fn failed_sync_item_report(subscription: &Subscription, err: SyncError) -> SyncItemReport {
    let risk_confirmation = match &err {
        SyncError::RiskConfirmationRequired {
            risk_level,
            permissions,
            ..
        } => Some(RiskConfirmationDetail {
            risk_level: *risk_level,
            permissions: permissions.clone(),
        }),
        _ => None,
    };
    SyncItemReport {
        workspace: subscription.workspace.clone(),
        asset_id: subscription.asset_id.clone(),
        ref_name: None,
        version: subscription.version.clone(),
        decision: UpdateDecision::Keep {
            reason: "sync failed".to_owned(),
        },
        source_path: None,
        source_hash: None,
        install: None,
        error: Some(err.to_string()),
        risk_confirmation,
    }
}

pub async fn rollback_asset(
    paths: &AppPaths,
    workspace: WorkspaceRef,
    asset_id: String,
    version: String,
    options: RollbackOptions,
) -> Result<RollbackReport> {
    ensure_local_state(paths)?;
    let targets = if options.targets.is_empty() {
        previous_targets(paths, &workspace, &asset_id)?
            .unwrap_or_else(|| TargetSelection::all_default().enabled_targets())
    } else {
        options.targets
    };
    let source = if let Some(source_override) = options.source_override {
        DownloadedSkillSource {
            source_dir: source_override,
            source_hash: None,
            ref_name: ref_name_for_version(&version),
        }
    } else {
        download_skill_source(
            paths,
            &workspace,
            &asset_id,
            &version,
            options.token.as_deref(),
        )
        .await?
    };
    let ref_name = source.ref_name.clone();
    ensure_risk_allowed(&source.source_dir, &asset_id, options.allow_risky)?;
    let install = skill_library_installer::install(InstallOptions {
        source_dir: source.source_dir.clone(),
        targets: targets.clone(),
        target_roots: options.target_roots,
    })?;
    let lock_path = workspace_lock_path(paths, &workspace);
    let lock = upsert_locked_asset(
        &lock_path,
        LockedAsset {
            workspace: workspace.clone(),
            asset_id: asset_id.clone(),
            version: version.clone(),
            ref_name: ref_name.clone(),
            pinned: true,
            installed_at: Utc::now(),
            targets,
        },
    )?;

    Ok(RollbackReport {
        workspace,
        asset_id,
        version,
        ref_name,
        source_path: source.source_dir,
        source_hash: source.source_hash,
        install,
        lock,
    })
}

async fn sync_subscription(
    paths: &AppPaths,
    subscription: &Subscription,
    options: &SyncOptions,
    token: Option<&str>,
) -> Result<SyncItemReport> {
    let target_names = subscription.targets.enabled_targets();
    let lock_path = workspace_lock_path(paths, &subscription.workspace);
    let current_lock = read_lockfile(&lock_path)?;
    let current = current_lock.assets.iter().find(|asset| {
        asset.workspace == subscription.workspace && asset.asset_id == subscription.asset_id
    });
    let pinned = current.map(|asset| asset.pinned).unwrap_or(false);
    let latest = match &subscription.version {
        Some(version) => version.clone(),
        None => {
            latest_version_for_subscription(&subscription.workspace, &subscription.asset_id, token)
                .await?
        }
    };
    let decision = decide_update(
        &subscription.update,
        current.map(|asset| asset.version.as_str()),
        &latest,
        pinned,
    )?;
    if !matches!(decision, UpdateDecision::Install { .. }) {
        return Ok(SyncItemReport {
            workspace: subscription.workspace.clone(),
            asset_id: subscription.asset_id.clone(),
            ref_name: Some(latest.clone()),
            version: Some(latest),
            decision,
            source_path: None,
            source_hash: None,
            install: None,
            error: None,
            risk_confirmation: None,
        });
    }

    let source = if let Some(source_override) = &options.source_override {
        DownloadedSkillSource {
            source_dir: source_override.clone(),
            source_hash: None,
            ref_name: ref_name_for_version(&latest),
        }
    } else {
        download_skill_source(
            paths,
            &subscription.workspace,
            &subscription.asset_id,
            &latest,
            token,
        )
        .await?
    };
    let ref_name = source.ref_name.clone();
    ensure_risk_allowed(
        &source.source_dir,
        &subscription.asset_id,
        options.allow_risky,
    )?;
    let install = skill_library_installer::install(InstallOptions {
        source_dir: source.source_dir.clone(),
        targets: target_names.clone(),
        target_roots: options.target_roots.clone(),
    })?;
    upsert_locked_asset(
        &lock_path,
        LockedAsset {
            workspace: subscription.workspace.clone(),
            asset_id: subscription.asset_id.clone(),
            version: latest.clone(),
            ref_name: ref_name.clone(),
            pinned: matches!(subscription.update, UpdatePolicy::Pin),
            installed_at: Utc::now(),
            targets: target_names,
        },
    )?;

    Ok(SyncItemReport {
        workspace: subscription.workspace.clone(),
        asset_id: subscription.asset_id.clone(),
        ref_name: Some(ref_name),
        version: Some(latest),
        decision,
        source_path: Some(source.source_dir),
        source_hash: source.source_hash,
        install: Some(install),
        error: None,
        risk_confirmation: None,
    })
}

fn previous_targets(
    paths: &AppPaths,
    workspace: &WorkspaceRef,
    asset_id: &str,
) -> Result<Option<Vec<String>>> {
    let lock_path = workspace_lock_path(paths, workspace);
    let lock = read_lockfile(&lock_path)?;
    Ok(lock
        .assets
        .into_iter()
        .find(|asset| asset.workspace == *workspace && asset.asset_id == asset_id)
        .map(|asset| asset.targets))
}

fn ensure_risk_allowed(source_dir: &Path, asset_id: &str, allow_risky: bool) -> Result<()> {
    let parse_result = skill_library_manifest::parse_skill_dir(source_dir)?;
    let manifest = parse_result
        .manifest
        .ok_or_else(|| SyncError::InvalidSource(format!("{:?}", parse_result.errors)))?;
    let risk_level = effective_risk(&manifest);
    if risk_level.requires_confirmation() && !allow_risky {
        return Err(SyncError::RiskConfirmationRequired {
            asset_id: asset_id.to_owned(),
            risk_level,
            permissions: manifest.permissions.join(", "),
        });
    }
    Ok(())
}

async fn latest_version_for_subscription(
    workspace: &WorkspaceRef,
    asset_id: &str,
    token: Option<&str>,
) -> Result<String> {
    let provider = github_provider(workspace, token)?;
    let tags = provider
        .list_tags(
            workspace,
            PageOpts {
                cursor: None,
                per_page: Some(100),
            },
        )
        .await
        .map_err(sync_provider_error)?;
    let best = tags
        .items
        .into_iter()
        .filter_map(|tag| {
            let version = tag.name.trim_start_matches('v');
            Version::parse(version)
                .ok()
                .map(|version| (version, tag.name))
        })
        .max_by(|(left, _), (right, _)| left.cmp(right));
    match best {
        Some((version, _tag)) => Ok(version.to_string()),
        None => {
            let detail = scan_github_workspace_detail(workspace, token).await?;
            detail
                .skills
                .into_iter()
                .find(|asset| skill_asset_matches(asset, asset_id))
                .map(|asset| asset.manifest.version)
                .ok_or_else(|| SyncError::NotFound(asset_id.to_owned()))
        }
    }
}

struct DownloadedSkillSource {
    source_dir: PathBuf,
    source_hash: Option<String>,
    /// The git ref actually downloaded (a tag when one matches the version,
    /// otherwise the repo's default branch).
    ref_name: String,
}

/// Resolve which git ref to download for a given version.
///
/// Public skill repos frequently have NO version tags — a single push often
/// changes several skills at once, so per-skill semver tagging isn't done. When
/// no tag matches we fall back to the repo's default branch (e.g. `main`).
/// A matching tag is preferred when present (reproducible historical installs).
async fn resolve_download_ref(
    provider: &GitHubProvider,
    workspace: &WorkspaceRef,
    version: &str,
) -> Result<String> {
    let normalized = version.trim_start_matches('v');
    let tags = provider
        .list_tags(
            workspace,
            PageOpts {
                cursor: None,
                per_page: Some(100),
            },
        )
        .await
        .map_err(sync_provider_error)?;
    if let Some(tag) = tags
        .items
        .into_iter()
        .find(|tag| tag.name == version || tag.name.trim_start_matches('v') == normalized)
    {
        return Ok(tag.name);
    }
    let ws = provider
        .get_workspace(workspace)
        .await
        .map_err(sync_provider_error)?;
    Ok(ws.default_branch)
}

fn fallback_download_refs(version: &str) -> Vec<String> {
    let version = version.trim();
    let mut refs = Vec::new();
    if !version.is_empty() {
        refs.push(version.to_owned());
        if !version.starts_with('v') {
            refs.push(format!("v{version}"));
        }
    }
    refs.push("HEAD".to_owned());
    refs.dedup();
    refs
}

async fn download_ref_candidates(
    provider: &GitHubProvider,
    workspace: &WorkspaceRef,
    version: &str,
    allow_api_resolution: bool,
) -> Result<Vec<String>> {
    if !allow_api_resolution {
        return Ok(fallback_download_refs(version));
    }

    match resolve_download_ref(provider, workspace, version).await {
        Ok(git_ref) => Ok(vec![git_ref]),
        Err(SyncError::Io(err)) if err.to_string().contains("rate limited") => {
            Ok(fallback_download_refs(version))
        }
        Err(err) => Err(err),
    }
}

async fn download_skill_source(
    paths: &AppPaths,
    workspace: &WorkspaceRef,
    asset_id: &str,
    version: &str,
    token: Option<&str>,
) -> Result<DownloadedSkillSource> {
    let provider = github_provider(workspace, token)?;
    let git_refs = download_ref_candidates(
        &provider,
        workspace,
        version,
        token.is_some_and(|token| !token.trim().is_empty()),
    )
    .await?;
    let mut last_error: Option<SyncError> = None;

    for git_ref in git_refs {
        let cache_dir = paths
            .workspaces
            .join(workspace.storage_key())
            .join("cache")
            .join(safe_filename(&git_ref));
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)?;
        }
        fs::create_dir_all(&cache_dir)?;
        match provider
            .download_tarball(workspace, &git_ref, &cache_dir)
            .await
        {
            Ok(archive) => {
                let assets = skill_library_manifest::scan_workspace(&archive.extracted_root)?;
                let asset = assets
                    .into_iter()
                    .find(|asset| skill_asset_matches(asset, asset_id))
                    .ok_or_else(|| SyncError::NotFound(asset_id.to_owned()))?;
                return Ok(DownloadedSkillSource {
                    source_dir: archive.extracted_root.join(asset.path),
                    source_hash: Some(format!("sha256:{}", archive.sha256)),
                    ref_name: git_ref,
                });
            }
            Err(err) => last_error = Some(sync_provider_error(err)),
        }
    }

    Err(last_error.unwrap_or_else(|| SyncError::NotFound(asset_id.to_owned())))
}

/// A skill downloaded from a workspace and located on disk, ready to be copied
/// into the local data directory and linked into agent tool folders.
#[derive(Debug, Clone)]
pub struct PreparedSkillDownload {
    /// Absolute path to the located skill directory inside the extracted tarball.
    pub source_dir: PathBuf,
    /// Path to the skill directory relative to the repository root.
    pub source_path: PathBuf,
    /// sha256 of the downloaded tarball (for provenance / change detection).
    pub source_hash: String,
    /// The git ref actually downloaded (a tag when matched, else default branch).
    pub ref_name: String,
    /// The skill's parsed manifest.
    pub manifest: skill_library_manifest::SkillManifest,
}

/// Download a single skill from a workspace and locate its directory, reporting
/// byte-level download progress via `on_progress(downloaded, total)`.
///
/// The skill is located either by the caller-supplied in-repo `skill_path`
/// (preferred — resolved by the discover detail view) or, failing that, by a
/// recursive workspace scan matching the SKILL.md metadata or path. The recursive scan is what
/// makes nested skills (e.g. `skills/category/the-skill/`) locatable; a flat
/// one-level scan was the cause of the historical "os error 3" install failures.
///
/// This is the building block for the async, progress-reporting install path. It
/// performs the network + extraction only; copying into the data dir and linking
/// into tool folders is the caller's responsibility.
pub async fn download_skill_for_install<F>(
    paths: &AppPaths,
    workspace: &WorkspaceRef,
    asset_id: &str,
    skill_path: Option<&str>,
    version: &str,
    token: Option<&str>,
    mut on_progress: F,
) -> Result<PreparedSkillDownload>
where
    F: FnMut(u64, Option<u64>),
{
    let provider = github_provider(workspace, token)?;
    let git_refs = download_ref_candidates(
        &provider,
        workspace,
        version,
        token.is_some_and(|token| !token.trim().is_empty()),
    )
    .await?;
    let mut last_error: Option<SyncError> = None;

    for git_ref in git_refs {
        let cache_dir = paths
            .workspaces
            .join(workspace.storage_key())
            .join("cache")
            .join(safe_filename(&git_ref));
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)?;
        }
        fs::create_dir_all(&cache_dir)?;
        let archive = match provider
            .download_tarball_with_progress(workspace, &git_ref, &cache_dir, &mut on_progress)
            .await
        {
            Ok(archive) => archive,
            Err(err) => {
                last_error = Some(sync_provider_error(err));
                continue;
            }
        };

        // Prefer the caller-supplied path (already resolved by discover detail).
        let by_path = skill_path
            .map(str::trim)
            .map(|p| p.trim_matches('/'))
            .filter(|p| !p.is_empty())
            .map(|p| archive.extracted_root.join(p))
            .filter(|dir| dir.exists());

        let (source_dir, manifest) = if let Some(dir) = by_path {
            let parsed = skill_library_manifest::parse_skill_dir(&dir)?;
            match parsed.manifest {
                Some(manifest) => (dir, manifest),
                // Path exists but isn't a valid skill -> fall back to id scan.
                None => locate_skill_by_id(&archive.extracted_root, asset_id)?,
            }
        } else {
            locate_skill_by_id(&archive.extracted_root, asset_id)?
        };

        let source_path = source_dir
            .strip_prefix(&archive.extracted_root)
            .unwrap_or(&source_dir)
            .to_path_buf();

        return Ok(PreparedSkillDownload {
            source_dir,
            source_path,
            source_hash: format!("sha256:{}", archive.sha256),
            ref_name: git_ref,
            manifest,
        });
    }

    Err(last_error.unwrap_or_else(|| SyncError::NotFound(asset_id.to_owned())))
}

/// Recursively scan an extracted workspace and return the skill matching
/// `asset_id` along with its parsed manifest.
fn locate_skill_by_id(
    extracted_root: &Path,
    asset_id: &str,
) -> Result<(PathBuf, skill_library_manifest::SkillManifest)> {
    let assets = skill_library_manifest::scan_workspace(extracted_root)?;
    let asset = assets
        .into_iter()
        .find(|asset| skill_asset_matches(asset, asset_id))
        .ok_or_else(|| SyncError::NotFound(asset_id.to_owned()))?;
    Ok((extracted_root.join(asset.path), asset.manifest))
}

fn skill_asset_matches(asset: &skill_library_manifest::SkillAsset, needle: &str) -> bool {
    let needle = needle.trim().trim_matches('/');
    if needle.is_empty() {
        return false;
    }

    let asset_path = asset.path.to_string_lossy().replace('\\', "/");
    let asset_basename = asset
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let name_id = normalize_skill_lookup_key(&asset.manifest.name);

    asset.manifest.id == needle
        || asset_path == needle
        || asset_basename == needle
        || asset.manifest.name.eq_ignore_ascii_case(needle)
        || name_id == normalize_skill_lookup_key(needle)
}

fn normalize_skill_lookup_key(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['-', '.', '_'])
        .to_owned()
}

/// A skill's full source tree, downloaded and cached locally, ready to feed to
/// the AI reviewer. The whole repo tarball is fetched once per resolved commit
/// and extracted under `workspaces/{key}/review-cache/{sha}`; we point at the
/// skill's subdirectory inside it.
#[derive(Debug, Clone)]
pub struct PreparedSkillReview {
    /// Absolute path to the skill's directory on disk.
    pub skill_dir: PathBuf,
    /// The git ref that was requested (branch/tag/sha), for display.
    pub ref_name: String,
    /// The immutable commit the cache is keyed on.
    pub commit_sha: String,
}

/// Download (or reuse a cached copy of) the entire skill source so it can be
/// reviewed in full — not just SKILL.md.
///
/// Caching is keyed on the skill directory's latest commit SHA rather than the
/// ref name: a tag/sha is immutable, but a branch advances, and reviewing a
/// stale cached copy while reporting "safe" would be dangerous. We resolve the
/// ref to the current commit touching this skill, and reuse the cache only when
/// that commit matches what we last extracted.
pub async fn prepare_skill_for_review(
    paths: &AppPaths,
    workspace: &WorkspaceRef,
    skill_path: &str,
    ref_name: Option<&str>,
    token: Option<&str>,
) -> Result<PreparedSkillReview> {
    let provider = github_provider(workspace, token)?;

    // Resolve the requested ref (or default branch) to a concrete name.
    let git_ref = match ref_name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => name.to_owned(),
        None => {
            provider
                .get_workspace(workspace)
                .await
                .map_err(sync_provider_error)?
                .default_branch
        }
    };

    let skill_path = skill_path.trim_matches('/');

    // Resolve to the latest commit touching this skill on that ref. This is the
    // immutable cache key; if it can't be resolved, fall back to the ref name.
    let commit_sha = provider
        .list_path_commits(workspace, skill_path, Some(&git_ref), 1)
        .await
        .ok()
        .and_then(|commits| commits.into_iter().next())
        .map(|commit| commit.sha)
        .unwrap_or_else(|| git_ref.clone());

    let cache_root = paths
        .workspaces
        .join(workspace.storage_key())
        .join("review-cache")
        .join(safe_filename(&commit_sha));

    // The extracted tarball root is `<cache_root>/<owner>-<repo>-<sha>/`; the
    // skill lives under that. We persist the extracted-root path in a marker so
    // a cache hit can locate it without re-listing.
    let marker = cache_root.join(".extracted-root");

    let extracted_root = if marker.exists() {
        let stored = fs::read_to_string(&marker)?;
        let root = PathBuf::from(stored.trim());
        if root.exists() {
            root
        } else {
            download_review_tarball(&provider, workspace, &git_ref, &cache_root, &marker).await?
        }
    } else {
        download_review_tarball(&provider, workspace, &git_ref, &cache_root, &marker).await?
    };

    let skill_dir = if skill_path.is_empty() {
        extracted_root
    } else {
        extracted_root.join(skill_path)
    };

    if !skill_dir.exists() {
        return Err(SyncError::NotFound(format!(
            "skill '{skill_path}' not found in {}",
            workspace.full_name()
        )));
    }

    Ok(PreparedSkillReview {
        skill_dir,
        ref_name: git_ref,
        commit_sha,
    })
}

/// Download and extract the repo tarball into `cache_root`, recording the
/// extracted-root path in `marker` for later cache hits.
async fn download_review_tarball(
    provider: &GitHubProvider,
    workspace: &WorkspaceRef,
    git_ref: &str,
    cache_root: &Path,
    marker: &Path,
) -> Result<PathBuf> {
    if cache_root.exists() {
        fs::remove_dir_all(cache_root)?;
    }
    fs::create_dir_all(cache_root)?;
    let archive = provider
        .download_tarball(workspace, git_ref, cache_root)
        .await
        .map_err(sync_provider_error)?;
    fs::write(marker, archive.extracted_root.to_string_lossy().as_bytes())?;
    Ok(archive.extracted_root)
}

/// Conventional ref placeholder for a version when no real git ref was
/// resolved (e.g. a local `source_override` that bypasses network download).
fn ref_name_for_version(version: &str) -> String {
    if version.starts_with('v') {
        version.to_owned()
    } else {
        format!("v{version}")
    }
}

fn safe_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn webhook_events(events: Vec<String>) -> Vec<String> {
    let mut cleaned = Vec::new();
    for event in events {
        let event = event.trim();
        if !event.is_empty() && !cleaned.iter().any(|existing| existing == event) {
            cleaned.push(event.to_owned());
        }
    }
    if cleaned.is_empty() {
        cleaned.push("push".to_owned());
    }
    cleaned
}

fn github_provider(workspace: &WorkspaceRef, token: Option<&str>) -> Result<GitHubProvider> {
    if workspace.provider != "github" {
        return Err(SyncError::NotFound(format!(
            "provider {} is not supported",
            workspace.provider
        )));
    }
    match token {
        Some(token) if !token.trim().is_empty() => {
            GitHubProvider::new(token.to_owned()).map_err(|err| {
                SyncError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    err.to_string(),
                ))
            })
        }
        _ => GitHubProvider::anonymous("https://api.github.com").map_err(|err| {
            SyncError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                err.to_string(),
            ))
        }),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        use std::io::Write;
        temp.write_all(bytes)?;
        temp.as_file_mut().sync_all()?;
        temp.persist(path).map_err(|err| SyncError::Io(err.error))?;
        Ok(())
    } else {
        fs::write(path, bytes)?;
        Ok(())
    }
}

fn sync_provider_error(err: ProviderError) -> SyncError {
    match err {
        ProviderError::NotFound { resource, .. } => SyncError::RemoteNotFound(resource),
        ProviderError::InvalidResponse(message)
        | ProviderError::ProviderUnavailable { message, .. }
        | ProviderError::Forbidden {
            reason: Some(message),
            ..
        }
        | ProviderError::NetworkError { cause: message } => {
            SyncError::Io(std::io::Error::new(std::io::ErrorKind::Other, message))
        }
        other => SyncError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            other.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SwapFailureInjection;

    impl SwapFailureInjection {
        fn enable() -> Self {
            std::env::set_var("SKILL_LIBRARY_INSTALLER_INJECT_SWAP_FAILURE", "1");
            Self
        }
    }

    impl Drop for SwapFailureInjection {
        fn drop(&mut self) {
            std::env::remove_var("SKILL_LIBRARY_INSTALLER_INJECT_SWAP_FAILURE");
        }
    }

    #[test]
    fn auto_patch_allows_patch_only() {
        assert_eq!(
            decide_update(&UpdatePolicy::AutoPatch, Some("1.2.3"), "1.2.4", false).unwrap(),
            UpdateDecision::Install {
                version: "1.2.4".to_owned()
            }
        );
        assert!(matches!(
            decide_update(&UpdatePolicy::AutoPatch, Some("1.2.3"), "1.3.0", false).unwrap(),
            UpdateDecision::Keep { .. }
        ));
    }

    #[test]
    fn auto_minor_allows_minor_within_same_major() {
        assert_eq!(
            decide_update(&UpdatePolicy::AutoMinor, Some("1.2.3"), "1.3.0", false).unwrap(),
            UpdateDecision::Install {
                version: "1.3.0".to_owned()
            }
        );
        assert_eq!(
            decide_update(&UpdatePolicy::AutoMinor, Some("1.2.3"), "1.2.4", false).unwrap(),
            UpdateDecision::Install {
                version: "1.2.4".to_owned()
            }
        );
        assert!(matches!(
            decide_update(&UpdatePolicy::AutoMinor, Some("1.2.3"), "2.0.0", false).unwrap(),
            UpdateDecision::Keep { .. }
        ));
    }

    #[test]
    fn manual_policy_reports_available_update_without_installing() {
        assert_eq!(
            decide_update(&UpdatePolicy::Manual, Some("1.2.3"), "1.2.4", false).unwrap(),
            UpdateDecision::Manual {
                version: "1.2.4".to_owned()
            }
        );
    }

    #[test]
    fn pin_and_locked_assets_keep_current_install() {
        assert_eq!(
            decide_update(&UpdatePolicy::Pin, Some("1.2.3"), "1.2.4", false).unwrap(),
            UpdateDecision::Keep {
                reason: "pinned".to_owned()
            }
        );
        assert_eq!(
            decide_update(&UpdatePolicy::AutoMinor, Some("1.2.3"), "1.2.4", true).unwrap(),
            UpdateDecision::Keep {
                reason: "pinned".to_owned()
            }
        );
    }

    #[test]
    fn missing_or_current_versions_do_not_create_false_updates() {
        assert_eq!(
            decide_update(&UpdatePolicy::Manual, None, "1.2.3", false).unwrap(),
            UpdateDecision::Install {
                version: "1.2.3".to_owned()
            }
        );
        assert_eq!(
            decide_update(&UpdatePolicy::AutoPatch, Some("1.2.3"), "1.2.3", false).unwrap(),
            UpdateDecision::Keep {
                reason: "already current".to_owned()
            }
        );
        assert_eq!(
            decide_update(&UpdatePolicy::AutoPatch, Some("1.2.3"), "1.2.2", false).unwrap(),
            UpdateDecision::Keep {
                reason: "latest is not newer".to_owned()
            }
        );
    }

    #[test]
    fn subscribe_replaces_existing_subscription() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subscriptions.yaml");
        let workspace = WorkspaceRef::github("acme", "team-skills");
        subscribe(
            &path,
            Subscription {
                workspace: workspace.clone(),
                asset_id: "code-reviewer".to_owned(),
                channel: "stable".to_owned(),
                version: None,
                update: UpdatePolicy::Manual,
                targets: TargetSelection::all_default(),
                subscribed_at: None,
            },
        )
        .unwrap();
        let file = subscribe(
            &path,
            Subscription {
                workspace,
                asset_id: "code-reviewer".to_owned(),
                channel: "stable".to_owned(),
                version: Some("1.0.0".to_owned()),
                update: UpdatePolicy::Pin,
                targets: TargetSelection::all_default(),
                subscribed_at: None,
            },
        )
        .unwrap();
        assert_eq!(file.subscriptions.len(), 1);
        assert_eq!(file.subscriptions[0].version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn default_target_selection_uses_claude_and_shared_agents_root() {
        assert_eq!(
            TargetSelection::all_default().enabled_targets(),
            vec!["claude-code".to_owned(), "codex".to_owned()]
        );
    }

    #[test]
    fn fallback_download_refs_try_version_tag_then_head() {
        assert_eq!(
            fallback_download_refs("1.2.3"),
            vec!["1.2.3".to_owned(), "v1.2.3".to_owned(), "HEAD".to_owned()]
        );
        assert_eq!(
            fallback_download_refs("v1.2.3"),
            vec!["v1.2.3".to_owned(), "HEAD".to_owned()]
        );
        assert_eq!(fallback_download_refs(""), vec!["HEAD".to_owned()]);
    }

    #[test]
    fn skill_asset_matches_skill_md_identity_path_and_name() {
        let root = tempfile::tempdir().unwrap();
        let skill_dir = root.path().join("skills/frontend-design");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: Frontend Design
description: Creates polished interfaces.
---
# Frontend Design
"#,
        )
        .unwrap();

        let asset = skill_library_manifest::scan_workspace(root.path())
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        assert!(skill_asset_matches(&asset, "frontend-design"));
        assert!(skill_asset_matches(&asset, "skills/frontend-design"));
        assert!(skill_asset_matches(&asset, "Frontend Design"));
    }

    #[test]
    fn subscribe_preserves_empty_targets_for_download_only() {
        // An empty target selection is a deliberate "download locally, deploy
        // nowhere" choice and must NOT be expanded to all tools.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subscriptions.yaml");
        let workspace = WorkspaceRef::github("acme", "team-skills");
        let file = subscribe(
            &path,
            Subscription {
                workspace,
                asset_id: "code-reviewer".to_owned(),
                channel: "stable".to_owned(),
                version: None,
                update: UpdatePolicy::Manual,
                targets: TargetSelection::default(),
                subscribed_at: None,
            },
        )
        .unwrap();
        assert_eq!(file.subscriptions.len(), 1);
        assert!(
            file.subscriptions[0].targets.enabled_targets().is_empty(),
            "empty targets must be preserved, not expanded to all tools"
        );
    }

    #[test]
    fn upsert_workspace_replaces_existing_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspaces.yaml");
        let mut first = stored_workspace("acme/team-skills", "read");
        first.webhook = Some(StoredWebhook {
            id: "123".to_owned(),
            callback_url: Some("https://team.example/api/webhooks/github".to_owned()),
            events: vec!["push".to_owned()],
            registered_at: Utc::now(),
        });
        upsert_workspace(&path, first.clone()).unwrap();
        first.permission = "admin".to_owned();
        first.default_branch = "trunk".to_owned();
        let file = upsert_workspace(&path, first).unwrap();

        assert_eq!(file.workspaces.len(), 1);
        assert_eq!(file.workspaces[0].permission, "admin");
        assert_eq!(file.workspaces[0].default_branch, "trunk");
        assert_eq!(file.workspaces[0].webhook.as_ref().unwrap().id, "123");
    }

    #[test]
    fn webhook_events_default_to_push_and_deduplicate() {
        assert_eq!(webhook_events(Vec::new()), vec!["push"]);
        assert_eq!(
            webhook_events(vec![
                "push".to_owned(),
                "release".to_owned(),
                "push".to_owned(),
                " ".to_owned(),
            ]),
            vec!["push", "release"]
        );
    }

    #[test]
    fn ensure_local_state_creates_registry_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::for_home(dir.path().join(".skill-library"));
        ensure_local_state(&paths).unwrap();

        assert!(paths.config.exists());
        assert!(paths.subscriptions.exists());
        assert!(paths.workspace_registry.exists());
        assert!(paths.workspaces.exists());
    }

    #[tokio::test]
    async fn sync_subscriptions_installs_source_override_and_writes_lock() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::for_home(dir.path().join(".skill-library"));
        ensure_local_state(&paths).unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::write(
            source.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews changes for correctness.
version: 1.2.3
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(source.path().join("SKILL.md"), "# Code Reviewer\n").unwrap();
        let target = tempfile::tempdir().unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");
        subscribe(
            &paths.subscriptions,
            Subscription {
                workspace: workspace.clone(),
                asset_id: "code-reviewer".to_owned(),
                channel: "stable".to_owned(),
                version: Some("1.2.3".to_owned()),
                update: UpdatePolicy::Manual,
                targets: TargetSelection {
                    claude_code: true,
                    cursor: false,
                    codex: false,
                    custom: Vec::new(),
                },
                subscribed_at: None,
            },
        )
        .unwrap();

        let report = sync_subscriptions(
            &paths,
            SyncOptions {
                token: None,
                target_roots: vec![TargetRoot {
                    target: "claude-code".to_owned(),
                    root: target.path().to_path_buf(),
                }],
                source_override: Some(source.path().to_path_buf()),
                allow_risky: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(report.items.len(), 1);
        assert!(report.items[0].install.is_some());
        assert!(target.path().join("code-reviewer/SKILL.md").exists());
        let lock = read_lockfile(workspace_lock_path(&paths, &workspace)).unwrap();
        assert_eq!(lock.assets[0].version, "1.2.3");
        assert_eq!(lock.assets[0].ref_name, "v1.2.3");
    }

    #[tokio::test]
    async fn sync_subscriptions_requires_confirmation_for_risky_source() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::for_home(dir.path().join(".skill-library"));
        ensure_local_state(&paths).unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::write(
            source.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews changes for correctness.
version: 1.2.3
targets:
  - claude-code
permissions:
  - shell.execute.limited
"#,
        )
        .unwrap();
        fs::write(source.path().join("SKILL.md"), "# Code Reviewer\n").unwrap();
        let target = tempfile::tempdir().unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");
        subscribe(
            &paths.subscriptions,
            Subscription {
                workspace: workspace.clone(),
                asset_id: "code-reviewer".to_owned(),
                channel: "stable".to_owned(),
                version: Some("1.2.3".to_owned()),
                update: UpdatePolicy::Manual,
                targets: TargetSelection {
                    claude_code: true,
                    cursor: false,
                    codex: false,
                    custom: Vec::new(),
                },
                subscribed_at: None,
            },
        )
        .unwrap();

        let report = sync_subscriptions(
            &paths,
            SyncOptions {
                token: None,
                target_roots: vec![TargetRoot {
                    target: "claude-code".to_owned(),
                    root: target.path().to_path_buf(),
                }],
                source_override: Some(source.path().to_path_buf()),
                allow_risky: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(report.items.len(), 1);
        assert!(report.items[0]
            .error
            .as_deref()
            .unwrap()
            .contains("risk confirmation required"));
        assert!(report.items[0].install.is_none());
        assert!(!target.path().join("code-reviewer/SKILL.md").exists());
        assert!(read_lockfile(workspace_lock_path(&paths, &workspace))
            .unwrap()
            .assets
            .is_empty());

        let confirmed = sync_subscriptions(
            &paths,
            SyncOptions {
                token: None,
                target_roots: vec![TargetRoot {
                    target: "claude-code".to_owned(),
                    root: target.path().to_path_buf(),
                }],
                source_override: Some(source.path().to_path_buf()),
                allow_risky: true,
            },
        )
        .await
        .unwrap();

        assert!(confirmed.items[0].install.is_some());
        assert!(target.path().join("code-reviewer/SKILL.md").exists());
    }

    #[tokio::test]
    async fn sync_subscriptions_restores_previous_install_and_keeps_lock_on_install_failure() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::for_home(dir.path().join(".skill-library"));
        ensure_local_state(&paths).unwrap();
        let target = tempfile::tempdir().unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");

        let source_v1 = tempfile::tempdir().unwrap();
        fs::write(
            source_v1.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews changes for correctness.
version: 1.0.0
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(source_v1.path().join("SKILL.md"), "# Code Reviewer v1\n").unwrap();
        subscribe(
            &paths.subscriptions,
            Subscription {
                workspace: workspace.clone(),
                asset_id: "code-reviewer".to_owned(),
                channel: "stable".to_owned(),
                version: Some("1.0.0".to_owned()),
                update: UpdatePolicy::AutoMinor,
                targets: TargetSelection {
                    claude_code: true,
                    cursor: false,
                    codex: false,
                    custom: Vec::new(),
                },
                subscribed_at: None,
            },
        )
        .unwrap();
        sync_subscriptions(
            &paths,
            SyncOptions {
                token: None,
                target_roots: vec![TargetRoot {
                    target: "claude-code".to_owned(),
                    root: target.path().to_path_buf(),
                }],
                source_override: Some(source_v1.path().to_path_buf()),
                allow_risky: false,
            },
        )
        .await
        .unwrap();

        let source_v2 = tempfile::tempdir().unwrap();
        fs::write(
            source_v2.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews changes for correctness.
version: 1.1.0
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(source_v2.path().join("SKILL.md"), "# Code Reviewer v2\n").unwrap();
        fs::write(source_v2.path().join(".skill-library-fail-swap"), "fail\n").unwrap();
        let _injection = SwapFailureInjection::enable();
        subscribe(
            &paths.subscriptions,
            Subscription {
                workspace: workspace.clone(),
                asset_id: "code-reviewer".to_owned(),
                channel: "stable".to_owned(),
                version: Some("1.1.0".to_owned()),
                update: UpdatePolicy::AutoMinor,
                targets: TargetSelection {
                    claude_code: true,
                    cursor: false,
                    codex: false,
                    custom: Vec::new(),
                },
                subscribed_at: None,
            },
        )
        .unwrap();

        let report = sync_subscriptions(
            &paths,
            SyncOptions {
                token: None,
                target_roots: vec![TargetRoot {
                    target: "claude-code".to_owned(),
                    root: target.path().to_path_buf(),
                }],
                source_override: Some(source_v2.path().to_path_buf()),
                allow_risky: false,
            },
        )
        .await
        .unwrap();

        assert!(report.items[0]
            .error
            .as_deref()
            .unwrap()
            .contains("previous version was restored"));
        assert_eq!(
            fs::read_to_string(target.path().join("code-reviewer/SKILL.md")).unwrap(),
            "# Code Reviewer v1\n"
        );
        let lock = read_lockfile(workspace_lock_path(&paths, &workspace)).unwrap();
        assert_eq!(lock.assets[0].version, "1.0.0");
        assert_eq!(lock.assets[0].ref_name, "v1.0.0");
    }

    #[tokio::test]
    async fn rollback_asset_installs_source_override_and_pins_lock() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::for_home(dir.path().join(".skill-library"));
        ensure_local_state(&paths).unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::write(
            source.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews changes for correctness.
version: 1.1.0
targets:
  - claude-code
"#,
        )
        .unwrap();
        fs::write(source.path().join("SKILL.md"), "# Code Reviewer\n").unwrap();
        let target = tempfile::tempdir().unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");

        let report = rollback_asset(
            &paths,
            workspace.clone(),
            "code-reviewer".to_owned(),
            "1.1.0".to_owned(),
            RollbackOptions {
                token: None,
                target_roots: vec![TargetRoot {
                    target: "claude-code".to_owned(),
                    root: target.path().to_path_buf(),
                }],
                source_override: Some(source.path().to_path_buf()),
                targets: vec!["claude-code".to_owned()],
                allow_risky: false,
            },
        )
        .await
        .unwrap();

        assert!(target.path().join("code-reviewer/SKILL.md").exists());
        assert_eq!(report.version, "1.1.0");
        assert_eq!(report.ref_name, "v1.1.0");
        assert!(report.source_hash.is_none());
        let lock = read_lockfile(workspace_lock_path(&paths, &workspace)).unwrap();
        assert_eq!(lock.assets[0].version, "1.1.0");
        assert_eq!(lock.assets[0].ref_name, "v1.1.0");
        assert!(lock.assets[0].pinned);
        assert_eq!(lock.assets[0].targets, vec!["claude-code"]);
    }

    #[tokio::test]
    async fn rollback_asset_requires_confirmation_for_risky_source() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::for_home(dir.path().join(".skill-library"));
        ensure_local_state(&paths).unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::write(
            source.path().join("manifest.yaml"),
            r#"
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews changes for correctness.
version: 1.1.0
targets:
  - claude-code
permissions:
  - network.external
"#,
        )
        .unwrap();
        fs::write(source.path().join("SKILL.md"), "# Code Reviewer\n").unwrap();
        let target = tempfile::tempdir().unwrap();
        let workspace = WorkspaceRef::github("acme", "team-skills");

        let err = rollback_asset(
            &paths,
            workspace.clone(),
            "code-reviewer".to_owned(),
            "1.1.0".to_owned(),
            RollbackOptions {
                token: None,
                target_roots: vec![TargetRoot {
                    target: "claude-code".to_owned(),
                    root: target.path().to_path_buf(),
                }],
                source_override: Some(source.path().to_path_buf()),
                targets: vec!["claude-code".to_owned()],
                allow_risky: false,
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(
            err,
            SyncError::RiskConfirmationRequired {
                risk_level: RiskLevel::High,
                ..
            }
        ));
        assert!(!target.path().join("code-reviewer/SKILL.md").exists());
        assert!(read_lockfile(workspace_lock_path(&paths, &workspace))
            .unwrap()
            .assets
            .is_empty());
    }

    fn stored_workspace(full_name: &str, permission: &str) -> StoredWorkspace {
        let (owner, repo) = full_name.split_once('/').unwrap();
        StoredWorkspace {
            provider: "github".to_owned(),
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            full_name: full_name.to_owned(),
            default_branch: "main".to_owned(),
            visibility: "private".to_owned(),
            permission: permission.to_owned(),
            html_url: Some(format!("https://github.com/{full_name}")),
            webhook: None,
            added_at: Utc::now(),
        }
    }
}
