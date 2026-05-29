use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use teamai_core::{
    AppPaths, GitHubCredential, RiskLevel, TeamAIConfig, UpdatePolicy, WorkspaceRef,
};
use teamai_installer::{InstallOptions, TargetRoot};
use teamai_manifest::SkillManifest;
use teamai_provider::{
    GitRef, Invitation, InvitationInput, PageOpts, PermissionLevel, Provider, PullRequest,
};
use teamai_provider_github::{GitHubProvider, GitHubPublishFile, GitHubPublishInput};
use teamai_sync::{Subscription, TargetSelection, WorkspaceWebhookRegistration};
use tracing_subscriber::{
    fmt::writer::{BoxMakeWriter, MakeWriterExt},
    EnvFilter,
};

#[derive(Debug, Parser)]
#[command(name = "teamai", version, about = "Team AI Hub CLI")]
struct Cli {
    #[arg(short, long, global = true, help = "Also write CLI logs to stderr")]
    verbose: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init,
    Login {
        #[command(subcommand)]
        command: LoginCommand,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Scan {
        path: PathBuf,
    },
    ScanRemote {
        workspace: String,
        #[arg(long)]
        token: Option<String>,
    },
    Subscribe {
        workspace: String,
        asset_id: String,
        #[arg(long, default_value = "manual")]
        update: CliUpdatePolicy,
        #[arg(long)]
        version: Option<String>,
        #[arg(long = "target")]
        targets: Vec<String>,
    },
    Unsubscribe {
        workspace: String,
        asset_id: String,
    },
    Subscriptions,
    Notifications {
        #[arg(long)]
        api: Option<String>,
        #[arg(long)]
        repository: Option<String>,
        #[arg(long)]
        since: Option<String>,
    },
    Sync {
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "target-root")]
        target_roots: Vec<String>,
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        pull_notifications: bool,
        #[arg(long)]
        api: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    Daemon {
        #[arg(long, default_value_t = 3600)]
        interval_seconds: u64,
        #[arg(long)]
        once: bool,
        #[arg(long)]
        token: Option<String>,
        #[arg(long = "target-root")]
        target_roots: Vec<String>,
        #[arg(long)]
        api: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    Status {
        #[arg(long = "target")]
        targets: Vec<String>,
        #[arg(long = "target-root")]
        target_roots: Vec<String>,
    },
    Versions {
        workspace: String,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long)]
        token: Option<String>,
    },
    Diff {
        workspace: String,
        from: String,
        to: String,
        #[arg(long)]
        skill_path: Option<String>,
        #[arg(long)]
        token: Option<String>,
    },
    Rollback {
        workspace: String,
        asset_id: String,
        version: String,
        #[arg(long = "target")]
        targets: Vec<String>,
        #[arg(long = "target-root")]
        target_roots: Vec<String>,
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    Invite {
        workspace: String,
        login: String,
        #[arg(long, default_value = "read")]
        role: CliInviteRole,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        api: Option<String>,
    },
    Install {
        source: PathBuf,
        #[arg(long = "target")]
        targets: Vec<String>,
        #[arg(long = "target-root")]
        target_roots: Vec<String>,
        #[arg(long)]
        yes: bool,
    },
    List {
        target: String,
        #[arg(long = "target-root")]
        target_roots: Vec<String>,
    },
    Remove {
        skill_id: String,
        #[arg(long = "target")]
        targets: Vec<String>,
        #[arg(long = "target-root")]
        target_roots: Vec<String>,
    },
    Package {
        source: PathBuf,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value = "local")]
        user: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        publish_pr: bool,
        #[arg(long)]
        auto_merge: bool,
        #[arg(long)]
        api: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    DecideUpdate {
        #[arg(long)]
        policy: CliUpdatePolicy,
        #[arg(long)]
        current: Option<String>,
        #[arg(long)]
        latest: String,
        #[arg(long)]
        pinned: bool,
    },
    Diagnostics {
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum LoginCommand {
    Github {
        #[arg(long)]
        token: Option<String>,
        #[arg(long, env = "GITHUB_CLIENT_ID")]
        client_id: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum AuthCommand {
    Status,
    Logout { provider: String },
}

#[derive(Debug, Subcommand)]
enum WorkspaceCommand {
    Add {
        workspace: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, env = "TEAMAI_WEBHOOK_CALLBACK_URL")]
        webhook_url: Option<String>,
        #[arg(long, env = "GITHUB_WEBHOOK_SECRET")]
        webhook_secret: Option<String>,
        #[arg(long = "webhook-event")]
        webhook_events: Vec<String>,
    },
    List,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliUpdatePolicy {
    AutoPatch,
    AutoMinor,
    Manual,
    Pin,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliInviteRole {
    Read,
    Triage,
    Write,
    Maintain,
    Admin,
}

impl From<CliInviteRole> for PermissionLevel {
    fn from(value: CliInviteRole) -> Self {
        match value {
            CliInviteRole::Read => Self::Read,
            CliInviteRole::Triage => Self::Triage,
            CliInviteRole::Write => Self::Write,
            CliInviteRole::Maintain => Self::Maintain,
            CliInviteRole::Admin => Self::Admin,
        }
    }
}

impl From<CliUpdatePolicy> for UpdatePolicy {
    fn from(value: CliUpdatePolicy) -> Self {
        match value {
            CliUpdatePolicy::AutoPatch => Self::AutoPatch,
            CliUpdatePolicy::AutoMinor => Self::AutoMinor,
            CliUpdatePolicy::Manual => Self::Manual,
            CliUpdatePolicy::Pin => Self::Pin,
        }
    }
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Login { .. } => "login",
            Self::Auth { .. } => "auth",
            Self::Workspace { .. } => "workspace",
            Self::Scan { .. } => "scan",
            Self::ScanRemote { .. } => "scan-remote",
            Self::Subscribe { .. } => "subscribe",
            Self::Unsubscribe { .. } => "unsubscribe",
            Self::Subscriptions => "subscriptions",
            Self::Notifications { .. } => "notifications",
            Self::Sync { .. } => "sync",
            Self::Daemon { .. } => "daemon",
            Self::Status { .. } => "status",
            Self::Versions { .. } => "versions",
            Self::Diff { .. } => "diff",
            Self::Rollback { .. } => "rollback",
            Self::Invite { .. } => "invite",
            Self::Install { .. } => "install",
            Self::List { .. } => "list",
            Self::Remove { .. } => "remove",
            Self::Package { .. } => "package",
            Self::DecideUpdate { .. } => "decide-update",
            Self::Diagnostics { .. } => "diagnostics",
        }
    }
}

fn init_logging(paths: &AppPaths, verbose: bool) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(&paths.logs)?;
    let log_path = cli_log_path(paths, chrono::Local::now().date_naive());
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open CLI log file {}", log_path.display()))?;
    let file_writer = Mutex::new(file);
    let writer = if verbose {
        BoxMakeWriter::new(file_writer.and(std::io::stderr))
    } else {
        BoxMakeWriter::new(file_writer)
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .init();
    Ok(log_path)
}

fn cli_log_path(paths: &AppPaths, date: chrono::NaiveDate) -> PathBuf {
    paths.logs.join(format!("{}.log", date.format("%Y-%m-%d")))
}

#[tokio::main]
async fn main() -> ExitCode {
    match run_cli().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", cli_error_json(&error));
            ExitCode::FAILURE
        }
    }
}

async fn run_cli() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let paths = AppPaths::resolve()?;
    let log_path = init_logging(&paths, cli.verbose)?;
    let command = cli.command.name();
    tracing::info!(
        command,
        log_path = %log_path.display(),
        "teamai command started"
    );
    let result = run_command(cli.command, &paths).await;
    match &result {
        Ok(()) => tracing::info!(command, "teamai command completed"),
        Err(error) => tracing::error!(command, error = %error, "teamai command failed"),
    }
    result
}

fn cli_error_json(error: &anyhow::Error) -> String {
    serde_json::to_string(&serde_json::json!({
        "ok": false,
        "error": {
            "code": cli_error_code(error),
            "message": error.to_string(),
        },
    }))
    .unwrap_or_else(|_| format!("Error: {error}"))
}

fn cli_error_code(error: &anyhow::Error) -> &'static str {
    if error.is::<teamai_provider::ProviderError>() {
        "provider_error"
    } else if error.is::<teamai_sync::SyncError>() {
        "sync_error"
    } else if error.is::<teamai_installer::InstallerError>() {
        "installer_error"
    } else if error.is::<teamai_manifest::ManifestError>() {
        "manifest_error"
    } else if error.is::<teamai_publish::PublishError>() {
        "publish_error"
    } else if error.is::<teamai_core::TeamAIError>() {
        "core_error"
    } else {
        "command_failed"
    }
}

async fn run_command(command: Command, paths: &AppPaths) -> anyhow::Result<()> {
    match command {
        Command::Init => {
            teamai_sync::ensure_local_state(&paths)?;
            println!("initialized {}", paths.home.display());
        }
        Command::Login { command } => match command {
            LoginCommand::Github { token, client_id } => {
                teamai_sync::ensure_local_state(&paths)?;
                let token = match token {
                    Some(token) => token,
                    None => login_github_device_flow(client_id).await?,
                };
                let provider = GitHubProvider::new(token.clone())?;
                let info = provider.validate_token().await?;
                teamai_core::save_github_credential(
                    &paths.credentials,
                    GitHubCredential {
                        token,
                        login: Some(info.user.login.clone()),
                        scopes: info.scopes.clone(),
                    },
                )?;
                println!("logged in to github as {}", info.user.login);
            }
        },
        Command::Auth { command } => {
            teamai_sync::ensure_local_state(&paths)?;
            match command {
                AuthCommand::Status => {
                    let status = teamai_core::load_github_credential(&paths.credentials)?
                        .and_then(|github| github.login)
                        .map(|login| format!("github: logged in as {login}"))
                        .unwrap_or_else(|| "github: not logged in".to_owned());
                    println!("{status}");
                }
                AuthCommand::Logout { provider } => {
                    if provider != "github" {
                        anyhow::bail!("unsupported provider `{provider}`");
                    }
                    teamai_core::delete_github_credential(&paths.credentials)?;
                    println!("logged out from github");
                }
            }
        }
        Command::Workspace { command } => {
            teamai_sync::ensure_local_state(&paths)?;
            match command {
                WorkspaceCommand::Add {
                    workspace,
                    token,
                    webhook_url,
                    webhook_secret,
                    webhook_events,
                } => {
                    let workspace = parse_workspace(&workspace)?;
                    let token = token.or_else(|| saved_github_token(&paths));
                    let webhook = workspace_webhook_registration(
                        webhook_url,
                        webhook_secret,
                        webhook_events,
                    )?;
                    let stored = teamai_sync::add_github_workspace_with_webhook(
                        &paths.workspace_registry,
                        &workspace,
                        token.as_deref(),
                        webhook,
                    )
                    .await?;
                    println!("{}", serde_json::to_string_pretty(&stored)?);
                }
                WorkspaceCommand::List => {
                    let file = teamai_sync::read_workspaces(&paths.workspace_registry)?;
                    println!("{}", serde_json::to_string_pretty(&file)?);
                }
            }
        }
        Command::Scan { path } => {
            let assets = teamai_manifest::scan_workspace(&path)
                .with_context(|| format!("failed to scan {}", path.display()))?;
            println!("{}", serde_json::to_string_pretty(&assets)?);
        }
        Command::ScanRemote { workspace, token } => {
            let workspace = parse_workspace(&workspace)?;
            let token = token.or_else(|| saved_github_token(&paths));
            let result =
                teamai_sync::scan_github_workspace_skills(&workspace, token.as_deref()).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::Subscribe {
            workspace,
            asset_id,
            update,
            version,
            targets,
        } => {
            teamai_sync::ensure_local_state(&paths)?;
            let workspace = parse_workspace(&workspace)?;
            let targets = selection_from_targets(targets);
            let file = teamai_sync::subscribe(
                &paths.subscriptions,
                Subscription {
                    workspace,
                    asset_id,
                    channel: "stable".to_owned(),
                    version,
                    update: update.into(),
                    targets,
                    subscribed_at: None,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&file)?);
        }
        Command::Unsubscribe {
            workspace,
            asset_id,
        } => {
            let workspace = parse_workspace(&workspace)?;
            let file = teamai_sync::unsubscribe(&paths.subscriptions, &workspace, &asset_id)?;
            println!("{}", serde_json::to_string_pretty(&file)?);
        }
        Command::Subscriptions => {
            let file = teamai_sync::read_subscriptions(&paths.subscriptions)?;
            println!("{}", serde_json::to_string_pretty(&file)?);
        }
        Command::Notifications {
            api,
            repository,
            since,
        } => {
            teamai_sync::ensure_local_state(&paths)?;
            let api = api.unwrap_or_else(|| read_config_or_default(&paths).api_base_url);
            let response =
                fetch_notifications(&api, repository.as_deref(), since.as_deref()).await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Command::Sync {
            token,
            target_roots,
            source,
            pull_notifications,
            api,
            yes,
        } => {
            teamai_sync::ensure_local_state(&paths)?;
            let api = api.unwrap_or_else(|| read_config_or_default(&paths).api_base_url);
            let notifications = if pull_notifications {
                Some(fetch_notifications(&api, None, None).await?)
            } else {
                None
            };
            let token = token.or_else(|| saved_github_token(&paths));
            let sync_options = teamai_sync::SyncOptions {
                token,
                target_roots: parse_target_roots(target_roots)?,
                source_override: source,
                allow_risky: yes,
            };
            let report = run_sync_with_risk_prompt(&paths, sync_options).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "notifications": notifications,
                    "sync": report,
                }))?
            );
        }
        Command::Daemon {
            interval_seconds,
            once,
            token,
            target_roots,
            api,
            yes,
        } => {
            teamai_sync::ensure_local_state(&paths)?;
            let api = api.unwrap_or_else(|| read_config_or_default(&paths).api_base_url);
            let token = token.or_else(|| saved_github_token(&paths));
            let target_roots = parse_target_roots(target_roots)?;
            run_daemon(
                &paths,
                token,
                target_roots,
                &api,
                interval_seconds,
                yes,
                once,
            )
            .await?;
        }
        Command::Status {
            targets,
            target_roots,
        } => {
            teamai_sync::ensure_local_state(&paths)?;
            let subscriptions = teamai_sync::read_subscriptions(&paths.subscriptions)?;
            let mut status = serde_json::json!({
                "subscriptions": subscriptions.subscriptions.clone(),
                "locks": [],
                "installed": {},
            });
            let mut workspace_refs = BTreeMap::new();
            for workspace in teamai_sync::read_workspaces(&paths.workspace_registry)?.workspaces {
                workspace_refs.insert(workspace.reference().storage_key(), workspace.reference());
            }
            for subscription in &subscriptions.subscriptions {
                workspace_refs.insert(
                    subscription.workspace.storage_key(),
                    subscription.workspace.clone(),
                );
            }
            let mut locks = Vec::new();
            for workspace in workspace_refs.values() {
                let lock_path = teamai_sync::workspace_lock_path(&paths, workspace);
                locks.push(serde_json::json!({
                    "workspace": workspace.full_name(),
                    "path": lock_path,
                    "lock": teamai_sync::read_lockfile(&lock_path)?,
                }));
            }
            status["locks"] = serde_json::Value::Array(locks);
            let target_roots = parse_target_roots(target_roots)?;
            let target_list = if targets.is_empty() {
                vec![
                    "claude-code".to_owned(),
                    "cursor".to_owned(),
                    "codex".to_owned(),
                ]
            } else {
                targets
            };
            let mut installed = serde_json::Map::new();
            for target in target_list {
                let value = teamai_installer::list_installed(&target, target_roots.clone())?;
                installed.insert(target, serde_json::to_value(value)?);
            }
            status["installed"] = serde_json::Value::Object(installed);
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Command::Versions {
            workspace,
            skill,
            token,
        } => {
            let workspace = parse_workspace(&workspace)?;
            let token = token.or_else(|| saved_github_token(&paths));
            let provider = github_provider(token.as_deref())?;
            let tags = provider
                .list_tags(
                    &workspace,
                    PageOpts {
                        cursor: None,
                        per_page: Some(100),
                    },
                )
                .await?;
            let detail = match skill {
                Some(skill_path) => Some(
                    teamai_sync::read_github_skill_detail(
                        &workspace,
                        &skill_path,
                        None,
                        token.as_deref(),
                    )
                    .await?,
                ),
                None => None,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "workspace": workspace,
                    "tags": tags.items,
                    "skill": detail,
                }))?
            );
        }
        Command::Diff {
            workspace,
            from,
            to,
            skill_path,
            token,
        } => {
            let workspace = parse_workspace(&workspace)?;
            let token = token.or_else(|| saved_github_token(&paths));
            let provider = github_provider(token.as_deref())?;
            let comparison = provider
                .compare_refs(
                    &workspace,
                    &GitRef::Tag(from.clone()),
                    &GitRef::Tag(to.clone()),
                )
                .await?;
            let semantic = match skill_path {
                Some(skill_path) => {
                    let from_detail = teamai_sync::read_github_skill_detail(
                        &workspace,
                        &skill_path,
                        Some(&from),
                        token.as_deref(),
                    )
                    .await?;
                    let to_detail = teamai_sync::read_github_skill_detail(
                        &workspace,
                        &skill_path,
                        Some(&to),
                        token.as_deref(),
                    )
                    .await?;
                    teamai_manifest::semantic_diff(
                        &from_detail.asset.manifest,
                        &to_detail.asset.manifest,
                    )
                }
                None => Vec::new(),
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "workspace": workspace,
                    "from": from,
                    "to": to,
                    "files": comparison.files,
                    "semantic": semantic,
                }))?
            );
        }
        Command::Rollback {
            workspace,
            asset_id,
            version,
            targets,
            target_roots,
            source,
            token,
            yes,
        } => {
            teamai_sync::ensure_local_state(&paths)?;
            let workspace = parse_workspace(&workspace)?;
            let token = token.or_else(|| saved_github_token(&paths));
            let targets = selection_from_targets(targets).enabled_targets();
            let report = run_rollback_with_risk_prompt(
                &paths,
                workspace,
                asset_id,
                version,
                teamai_sync::RollbackOptions {
                    token,
                    target_roots: parse_target_roots(target_roots)?,
                    source_override: source,
                    targets,
                    allow_risky: yes,
                },
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "rollback": report,
                    "note": "rollback installed the requested version and pinned the local lockfile",
                }))?
            );
        }
        Command::Invite {
            workspace,
            login,
            role,
            token,
            api,
        } => {
            teamai_sync::ensure_local_state(&paths)?;
            let api = api.unwrap_or_else(|| read_config_or_default(&paths).api_base_url);
            let workspace = parse_workspace(&workspace)?;
            let role_value: PermissionLevel = role.into();
            let token = token
                .or_else(|| saved_github_token(&paths))
                .ok_or_else(|| anyhow::anyhow!("GitHub token is required for invite"))?;
            let provider = GitHubProvider::new(token)?;
            let current_user = provider.current_user().await?;
            let permission = provider
                .check_permission(&workspace, &current_user.login)
                .await?;
            if !matches!(
                permission,
                PermissionLevel::Admin | PermissionLevel::Maintain
            ) {
                anyhow::bail!(
                    "github user {} must have admin or maintain permission on {} to invite collaborators",
                    current_user.login,
                    workspace.full_name()
                );
            }
            let invitation = provider
                .create_invitation(
                    &workspace,
                    InvitationInput {
                        login_or_email: login.clone(),
                        role: role_value.clone(),
                    },
                )
                .await?;
            let api_record = post_invitation_record(
                &api,
                &workspace.full_name(),
                &login,
                &role_value,
                &invitation,
            )
            .await;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "invitation": invitation,
                    "apiRecord": api_record,
                }))?
            );
        }
        Command::Install {
            source,
            targets,
            target_roots,
            yes,
        } => {
            confirm_skill_source_risk("install", &source, yes)?;
            let report = teamai_installer::install(InstallOptions {
                source_dir: source,
                targets,
                target_roots: parse_target_roots(target_roots)?,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::List {
            target,
            target_roots,
        } => {
            let installed =
                teamai_installer::list_installed(&target, parse_target_roots(target_roots)?)?;
            println!("{}", serde_json::to_string_pretty(&installed)?);
        }
        Command::Remove {
            skill_id,
            targets,
            target_roots,
        } => {
            let removed =
                teamai_installer::remove(&skill_id, &targets, parse_target_roots(target_roots)?)?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::Package {
            source,
            workspace,
            user,
            token,
            publish_pr,
            auto_merge,
            api,
            yes,
        } => {
            let package = teamai_publish::package_skill(&source)?;
            let policy = teamai_publish::evaluate_publish_policy(&package)?;
            if let Some(workspace) = workspace {
                let workspace = parse_workspace(&workspace)?;
                let request = teamai_publish::build_publish_request(&package, &workspace, &user);
                if publish_pr {
                    confirm_manifest_risk(
                        "publish PR",
                        &package.manifest,
                        package.risk_level,
                        yes,
                    )?;
                    teamai_sync::ensure_local_state(&paths)?;
                    let api = api.unwrap_or_else(|| read_config_or_default(&paths).api_base_url);
                    let token = token
                        .or_else(|| saved_github_token(&paths))
                        .ok_or_else(|| {
                            anyhow::anyhow!("GitHub token is required for --publish-pr")
                        })?;
                    let provider = GitHubProvider::new(token)?;
                    let current_user = provider.current_user().await?;
                    let permission = provider
                        .check_permission(&workspace, &current_user.login)
                        .await?;
                    if !matches!(
                        permission,
                        PermissionLevel::Admin | PermissionLevel::Maintain | PermissionLevel::Write
                    ) {
                        anyhow::bail!(
                            "github user {} does not have write permission on {}",
                            current_user.login,
                            workspace.full_name()
                        );
                    }
                    if matches!(
                        policy.decision,
                        teamai_publish::PublishPolicyDecision::Reject
                    ) {
                        anyhow::bail!(
                            "publish policy rejected {}: {}",
                            package.manifest.id,
                            policy.reasons.join("; ")
                        );
                    }
                    let files = teamai_publish::collect_publish_files(&package)?
                        .into_iter()
                        .map(|file| GitHubPublishFile {
                            path: file.target_path,
                            bytes: file.bytes,
                        })
                        .collect();
                    let result = provider
                        .publish_files_pull_request(
                            &workspace,
                            GitHubPublishInput {
                                branch_name: request.branch_name.clone(),
                                commit_message: request.title.clone(),
                                title: request.title.clone(),
                                body: request.body.clone(),
                                base: None,
                                files,
                            },
                        )
                        .await?;
                    let merge = if auto_merge {
                        if !policy.auto_merge_allowed {
                            anyhow::bail!(
                                "--auto-merge requested but policy requires review: {}",
                                policy.reasons.join("; ")
                            );
                        }
                        if !matches!(
                            permission,
                            PermissionLevel::Admin | PermissionLevel::Maintain
                        ) {
                            anyhow::bail!(
                                "--auto-merge requires maintain/admin permission on {}",
                                workspace.full_name()
                            );
                        }
                        Some(
                            provider
                                .merge_pull_request(&workspace, result.pull_request.number)
                                .await?,
                        )
                    } else {
                        None
                    };
                    let api_record = post_publish_request_record(
                        &api,
                        &workspace.full_name(),
                        &package,
                        &user,
                        &policy,
                        Some(&result.pull_request),
                        merge.is_some(),
                    )
                    .await;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "request": request,
                            "policy": policy,
                            "result": result,
                            "merge": merge,
                            "apiRecord": api_record,
                        }))?
                    );
                } else {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "request": request,
                            "policy": policy,
                        }))?
                    );
                }
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "package": package,
                        "policy": policy,
                    }))?
                );
            }
        }
        Command::DecideUpdate {
            policy,
            current,
            latest,
            pinned,
        } => {
            let decision =
                teamai_sync::decide_update(&policy.into(), current.as_deref(), &latest, pinned)?;
            println!("{}", serde_json::to_string_pretty(&decision)?);
        }
        Command::Diagnostics { output } => {
            teamai_sync::ensure_local_state(&paths)?;
            let report = export_diagnostics(paths, output)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}

fn parse_workspace(value: &str) -> anyhow::Result<WorkspaceRef> {
    let value = value
        .trim()
        .strip_prefix("github.com/")
        .unwrap_or_else(|| value.trim());
    let Some((owner, repo)) = value.split_once('/') else {
        anyhow::bail!("workspace must look like owner/repo or github.com/owner/repo");
    };
    Ok(WorkspaceRef::github(owner, repo))
}

fn saved_github_token(paths: &AppPaths) -> Option<String> {
    teamai_core::load_github_credential(&paths.credentials)
        .ok()
        .flatten()
        .map(|github| github.token)
}

fn github_provider(token: Option<&str>) -> anyhow::Result<GitHubProvider> {
    match token {
        Some(token) if !token.trim().is_empty() => Ok(GitHubProvider::new(token.to_owned())?),
        _ => Ok(GitHubProvider::anonymous("https://api.github.com")?),
    }
}

async fn fetch_notifications(
    api_base_url: &str,
    repository: Option<&str>,
    since: Option<&str>,
) -> anyhow::Result<NotificationsResponse> {
    let base = api_base_url.trim_end_matches('/');
    let mut url = reqwest::Url::parse(&format!("{base}/api/notifications"))?;
    {
        let mut query = url.query_pairs_mut();
        if let Some(repository) = repository.filter(|value| !value.trim().is_empty()) {
            query.append_pair("repository", repository);
        }
        if let Some(since) = since.filter(|value| !value.trim().is_empty()) {
            query.append_pair("since", since);
        }
    }
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to fetch notifications from {api_base_url}"))?;
    let status = response.status();
    if !status.is_success() {
        let message = response.text().await.unwrap_or_else(|_| status.to_string());
        anyhow::bail!(
            "notification request failed with {status}: {}",
            friendly_api_error(&message)
        );
    }
    Ok(response.json().await?)
}

async fn post_publish_request_record(
    api_base_url: &str,
    workspace: &str,
    package: &teamai_publish::PublishPackage,
    source_user: &str,
    policy: &teamai_publish::PublishPolicyResult,
    pull_request: Option<&PullRequest>,
    auto_merged: bool,
) -> serde_json::Value {
    let pull_request = pull_request.map(|pr| {
        serde_json::json!({
            "number": pr.number,
            "title": pr.title,
            "htmlUrl": pr.html_url,
            "state": pr.state,
        })
    });
    let response = post_api_json(
        api_base_url,
        "/api/publish-requests",
        serde_json::json!({
            "workspace": workspace,
            "skillId": package.manifest.id,
            "skillVersion": package.manifest.version,
            "sourceUser": source_user,
            "sourcePath": package.source_path,
            "sourceHash": package.source_hash,
            "pullRequest": pull_request,
            "policy": policy,
        }),
    )
    .await;
    match response {
        Ok(mut value) => {
            if auto_merged {
                if let Some(id) = value
                    .get("publishRequest")
                    .and_then(|record| record.get("id"))
                    .and_then(|id| id.as_str())
                {
                    if let Ok(updated) = post_api_json(
                        api_base_url,
                        &format!("/api/publish-requests/{id}"),
                        serde_json::json!({
                            "state": "merged",
                            "autoMerged": true,
                        }),
                    )
                    .await
                    {
                        value["autoMergeUpdate"] = updated;
                    }
                }
            }
            value
        }
        Err(err) => serde_json::json!({ "error": err.to_string() }),
    }
}

async fn post_invitation_record(
    api_base_url: &str,
    workspace: &str,
    invitee: &str,
    role: &PermissionLevel,
    invitation: &Invitation,
) -> serde_json::Value {
    match post_api_json(
        api_base_url,
        "/api/invitations",
        serde_json::json!({
            "workspace": workspace,
            "invitee": invitee,
            "role": permission_level_name(role),
            "providerInvitationId": invitation.id,
            "state": invitation.state,
        }),
    )
    .await
    {
        Ok(value) => value,
        Err(err) => serde_json::json!({ "error": err.to_string() }),
    }
}

async fn post_api_json(
    api_base_url: &str,
    path: &str,
    body: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let base = api_base_url.trim_end_matches('/');
    let url = format!("{base}{path}");
    let response = reqwest::Client::new()
        .request(api_method_for_path(path), &url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("failed to post {url}"))?;
    let status = response.status();
    if !status.is_success() {
        let message = response.text().await.unwrap_or_else(|_| status.to_string());
        anyhow::bail!(
            "api request failed with {status}: {}",
            friendly_api_error(&message)
        );
    }
    Ok(response.json().await?)
}

pub fn friendly_api_error(raw: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return raw.to_owned();
    };
    match value.get("error") {
        Some(serde_json::Value::String(code)) => code.to_owned(),
        Some(serde_json::Value::Object(error)) => {
            let code = error.get("code").and_then(|value| value.as_str());
            let message = error.get("message").and_then(|value| value.as_str());
            match (code, message) {
                (Some(code), Some(message)) => format!("{code}: {message}"),
                (None, Some(message)) => message.to_owned(),
                (Some(code), None) => code.to_owned(),
                (None, None) => raw.to_owned(),
            }
        }
        _ => raw.to_owned(),
    }
}

fn api_method_for_path(path: &str) -> reqwest::Method {
    if path.contains("/api/publish-requests/") || path.contains("/api/invitations/") {
        reqwest::Method::PATCH
    } else {
        reqwest::Method::POST
    }
}

fn permission_level_name(role: &PermissionLevel) -> &'static str {
    match role {
        PermissionLevel::Admin => "admin",
        PermissionLevel::Maintain => "maintain",
        PermissionLevel::Write => "write",
        PermissionLevel::Triage => "triage",
        PermissionLevel::Read => "read",
        PermissionLevel::None => "none",
    }
}

async fn run_daemon(
    paths: &AppPaths,
    token: Option<String>,
    target_roots: Vec<TargetRoot>,
    api_base_url: &str,
    interval_seconds: u64,
    yes: bool,
    once: bool,
) -> anyhow::Result<()> {
    let interval = std::time::Duration::from_secs(interval_seconds.max(1));
    loop {
        let daemon = run_daemon_once(
            paths,
            token.clone(),
            target_roots.clone(),
            api_base_url,
            interval.as_secs(),
            yes,
        )
        .await?;
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({ "daemon": daemon }))?
        );
        if once {
            return Ok(());
        }
        tokio::time::sleep(interval).await;
    }
}

async fn run_daemon_once(
    paths: &AppPaths,
    token: Option<String>,
    target_roots: Vec<TargetRoot>,
    api_base_url: &str,
    next_poll_seconds: u64,
    yes: bool,
) -> anyhow::Result<DaemonPollReport> {
    let notifications = match fetch_notifications(api_base_url, None, None).await {
        Ok(notifications) => notifications,
        Err(err) => NotificationsResponse {
            notifications: Vec::new(),
            error: Some(err.to_string()),
        },
    };
    let sync = run_sync_with_risk_prompt(
        paths,
        teamai_sync::SyncOptions {
            token,
            target_roots,
            source_override: None,
            allow_risky: yes,
        },
    )
    .await?;
    Ok(DaemonPollReport {
        notifications,
        sync,
        next_poll_seconds,
    })
}

async fn run_sync_with_risk_prompt(
    paths: &AppPaths,
    mut options: teamai_sync::SyncOptions,
) -> anyhow::Result<teamai_sync::SyncReport> {
    let report = teamai_sync::sync_subscriptions(paths, options.clone()).await?;
    let requests = report.risk_confirmation_requests();
    if options.allow_risky || requests.is_empty() {
        return Ok(report);
    }

    for request in &requests {
        confirm_risk_summary(
            "sync",
            &request.asset_id,
            request.risk_level,
            &request.permissions,
            false,
        )?;
    }
    options.allow_risky = true;
    Ok(teamai_sync::sync_subscriptions(paths, options).await?)
}

async fn run_rollback_with_risk_prompt(
    paths: &AppPaths,
    workspace: WorkspaceRef,
    asset_id: String,
    version: String,
    mut options: teamai_sync::RollbackOptions,
) -> anyhow::Result<teamai_sync::RollbackReport> {
    match teamai_sync::rollback_asset(
        paths,
        workspace.clone(),
        asset_id.clone(),
        version.clone(),
        options.clone(),
    )
    .await
    {
        Ok(report) => Ok(report),
        Err(teamai_sync::SyncError::RiskConfirmationRequired {
            risk_level,
            permissions,
            ..
        }) if !options.allow_risky => {
            confirm_risk_summary("rollback", &asset_id, risk_level, &permissions, false)?;
            options.allow_risky = true;
            Ok(teamai_sync::rollback_asset(paths, workspace, asset_id, version, options).await?)
        }
        Err(err) => Err(err.into()),
    }
}

fn read_config_or_default(paths: &AppPaths) -> TeamAIConfig {
    std::fs::read_to_string(&paths.config)
        .ok()
        .and_then(|raw| toml::from_str::<TeamAIConfig>(&raw).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsReport {
    exported_at: chrono::DateTime<chrono::Utc>,
    output_dir: PathBuf,
    app_home: PathBuf,
    config: serde_json::Value,
    subscriptions: usize,
    workspaces: Vec<DiagnosticsWorkspace>,
    logs: Vec<PathBuf>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsWorkspace {
    full_name: String,
    provider: String,
    lock_path: PathBuf,
    locked_assets: usize,
}

fn export_diagnostics(
    paths: &AppPaths,
    output: Option<PathBuf>,
) -> anyhow::Result<DiagnosticsReport> {
    let exported_at = chrono::Utc::now();
    let output_dir = output.unwrap_or_else(|| {
        paths
            .tmp
            .join("diagnostics")
            .join(exported_at.format("%Y%m%dT%H%M%SZ").to_string())
    });
    fs::create_dir_all(&output_dir)?;

    let config = read_sanitized_config(paths)?;
    let subscriptions = teamai_sync::read_subscriptions(&paths.subscriptions)?;
    let workspaces = teamai_sync::read_workspaces(&paths.workspace_registry)?;
    let mut workspace_summaries = Vec::new();
    for workspace in &workspaces.workspaces {
        let reference = workspace.reference();
        let lock_path = teamai_sync::workspace_lock_path(paths, &reference);
        let lock = teamai_sync::read_lockfile(&lock_path)?;
        workspace_summaries.push(DiagnosticsWorkspace {
            full_name: workspace.full_name.clone(),
            provider: workspace.provider.clone(),
            lock_path,
            locked_assets: lock.assets.len(),
        });
    }

    fs::write(
        output_dir.join("summary.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "exportedAt": exported_at,
            "appHome": paths.home,
            "config": config,
            "subscriptionCount": subscriptions.subscriptions.len(),
            "workspaceCount": workspaces.workspaces.len(),
            "workspaces": workspace_summaries,
        }))?,
    )?;
    fs::write(
        output_dir.join("subscriptions.json"),
        serde_json::to_vec_pretty(&subscriptions)?,
    )?;
    fs::write(
        output_dir.join("workspaces.json"),
        serde_json::to_vec_pretty(&workspaces)?,
    )?;

    let copied_logs = copy_sanitized_logs(&paths.logs, &output_dir.join("logs"))?;
    let report = DiagnosticsReport {
        exported_at,
        output_dir,
        app_home: paths.home.clone(),
        config,
        subscriptions: subscriptions.subscriptions.len(),
        workspaces: workspace_summaries,
        logs: copied_logs,
        notes: vec![
            "credentials.json and OS keychain secrets are intentionally excluded".to_owned(),
            "log files are copied with token-looking values redacted".to_owned(),
        ],
    };
    fs::write(
        report.output_dir.join("diagnostics.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}

fn read_sanitized_config(paths: &AppPaths) -> anyhow::Result<serde_json::Value> {
    let config = read_config_or_default(paths);
    Ok(serde_json::to_value(config)?)
}

fn copy_sanitized_logs(logs_dir: &Path, output_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    if !logs_dir.exists() {
        return Ok(Vec::new());
    }
    fs::create_dir_all(output_dir)?;
    let mut copied = Vec::new();
    for entry in fs::read_dir(logs_dir)? {
        let entry = entry?;
        let source = entry.path();
        if !source.is_file() || source.extension().and_then(|value| value.to_str()) != Some("log") {
            continue;
        }
        let Some(file_name) = source.file_name() else {
            continue;
        };
        let destination = output_dir.join(file_name);
        let raw = fs::read_to_string(&source).unwrap_or_else(|_| "<binary log omitted>".to_owned());
        fs::write(&destination, redact_sensitive_text(&raw))?;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotificationsResponse {
    notifications: Vec<NotificationEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NotificationEvent {
    id: String,
    kind: String,
    provider: String,
    repository: String,
    #[serde(rename = "ref")]
    #[serde(default)]
    ref_name: Option<String>,
    #[serde(default)]
    after: Option<String>,
    source_event: String,
    #[serde(default)]
    delivery: Option<String>,
    received_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DaemonPollReport {
    notifications: NotificationsResponse,
    sync: teamai_sync::SyncReport,
    next_poll_seconds: u64,
}

async fn login_github_device_flow(client_id: Option<String>) -> anyhow::Result<String> {
    let client_id = client_id.ok_or_else(|| {
        anyhow::anyhow!(
            "missing GitHub OAuth client id; pass --client-id or set GITHUB_CLIENT_ID, or use --token"
        )
    })?;
    let device =
        GitHubProvider::start_device_flow(&client_id, &["repo", "read:org", "read:user"]).await?;
    println!(
        "Open {} and enter code {}",
        device.verification_uri, device.user_code
    );
    let interval = std::time::Duration::from_secs(device.interval.max(1));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(device.expires_in);
    loop {
        if std::time::Instant::now() > deadline {
            anyhow::bail!("device code expired");
        }
        tokio::time::sleep(interval).await;
        let response = GitHubProvider::poll_device_flow(&client_id, &device.device_code).await?;
        match (response.access_token, response.error.as_deref()) {
            (Some(token), _) => return Ok(token),
            (None, Some("authorization_pending")) => continue,
            (None, Some("slow_down")) => {
                tokio::time::sleep(interval).await;
                continue;
            }
            (None, Some(error)) => {
                anyhow::bail!(
                    "{}",
                    response
                        .error_description
                        .unwrap_or_else(|| error.to_owned())
                )
            }
            (None, None) => continue,
        }
    }
}

fn selection_from_targets(targets: Vec<String>) -> TargetSelection {
    if targets.is_empty() {
        return TargetSelection::all_default();
    }
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

fn workspace_webhook_registration(
    webhook_url: Option<String>,
    webhook_secret: Option<String>,
    webhook_events: Vec<String>,
) -> anyhow::Result<Option<WorkspaceWebhookRegistration>> {
    match (webhook_url, webhook_secret) {
        (Some(callback_url), Some(secret)) => Ok(Some(WorkspaceWebhookRegistration {
            callback_url,
            secret,
            events: webhook_events,
        })),
        (None, None) if webhook_events.is_empty() => Ok(None),
        (None, _) => {
            anyhow::bail!("--webhook-url is required when registering a workspace webhook")
        }
        (_, None) => {
            anyhow::bail!("--webhook-secret is required when registering a workspace webhook")
        }
    }
}

fn confirm_skill_source_risk(action: &str, source: &PathBuf, yes: bool) -> anyhow::Result<()> {
    let parse_result = teamai_manifest::parse_skill_dir(source)?;
    let manifest = parse_result
        .manifest
        .ok_or_else(|| anyhow::anyhow!("invalid skill source: {:?}", parse_result.errors))?;
    let risk_level = teamai_manifest::effective_risk(&manifest);
    confirm_manifest_risk(action, &manifest, risk_level, yes)
}

fn confirm_manifest_risk(
    action: &str,
    manifest: &SkillManifest,
    risk_level: RiskLevel,
    yes: bool,
) -> anyhow::Result<()> {
    if !risk_level.requires_confirmation() {
        return Ok(());
    }
    if yes {
        return Ok(());
    }

    confirm_risk_summary(
        action,
        &manifest.id,
        risk_level,
        &permission_summary(manifest),
        false,
    )
}

fn confirm_risk_summary(
    action: &str,
    asset_id: &str,
    risk_level: RiskLevel,
    permissions: &str,
    yes: bool,
) -> anyhow::Result<()> {
    if yes {
        return Ok(());
    }

    eprintln!(
        "{} `{}` declares {} risk permissions: {}",
        action, asset_id, risk_level, permissions
    );
    eprint!("Continue? [y/N] ");
    std::io::stderr().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        anyhow::bail!(
            "{} cancelled; rerun with --yes to confirm {} risk",
            action,
            risk_level
        );
    }
}

fn permission_summary(manifest: &SkillManifest) -> String {
    if manifest.permissions.is_empty() {
        "none declared".to_owned()
    } else {
        manifest.permissions.join(", ")
    }
}

fn parse_target_roots(values: Vec<String>) -> anyhow::Result<Vec<TargetRoot>> {
    values
        .into_iter()
        .map(|value| {
            let Some((target, root)) = value.split_once('=') else {
                anyhow::bail!("target root must look like target=/path");
            };
            Ok(TargetRoot {
                target: target.to_owned(),
                root: PathBuf::from(root),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        cli_error_code, cli_error_json, cli_log_path, friendly_api_error, redact_sensitive_text,
        Command, DaemonPollReport, NotificationsResponse,
    };

    #[test]
    fn friendly_api_error_formats_structured_error_envelopes() {
        let raw = r#"{"ok":false,"error":{"code":"invalid_request","message":"The request body does not match the Team AI Hub API contract."}}"#;

        assert_eq!(
            friendly_api_error(raw),
            "invalid_request: The request body does not match the Team AI Hub API contract."
        );
    }

    #[test]
    fn friendly_api_error_keeps_legacy_error_codes_readable() {
        assert_eq!(
            friendly_api_error(r#"{"ok":false,"error":"not_found"}"#),
            "not_found"
        );
        assert_eq!(
            friendly_api_error("upstream unavailable"),
            "upstream unavailable"
        );
    }

    #[test]
    fn daemon_poll_report_uses_cli_json_contract() {
        let report = DaemonPollReport {
            notifications: NotificationsResponse {
                notifications: Vec::new(),
                error: Some("offline".to_owned()),
            },
            sync: teamai_sync::SyncReport {
                synced_at: chrono::DateTime::parse_from_rfc3339("2026-05-27T00:00:00Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc),
                items: Vec::new(),
            },
            next_poll_seconds: 60,
        };

        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["nextPollSeconds"], 60);
        assert_eq!(value["notifications"]["error"], "offline");
        assert_eq!(value["sync"]["items"], serde_json::json!([]));
    }

    #[test]
    fn command_name_does_not_include_sensitive_args() {
        let command = Command::Login {
            command: super::LoginCommand::Github {
                token: Some("secret-token".to_owned()),
                client_id: None,
            },
        };

        assert_eq!(command.name(), "login");
    }

    #[test]
    fn cli_log_path_uses_date_file_under_app_logs() {
        let paths = teamai_core::AppPaths::for_home(std::path::PathBuf::from("/tmp/teamai"));
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();

        assert_eq!(
            cli_log_path(&paths, date),
            std::path::PathBuf::from("/tmp/teamai/logs/2026-05-27.log")
        );
    }

    #[test]
    fn cli_error_json_uses_structured_error_envelope() {
        let error = anyhow::anyhow!("workspace must look like owner/repo");
        let value: serde_json::Value = serde_json::from_str(&cli_error_json(&error)).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "command_failed");
        assert_eq!(
            value["error"]["message"],
            "workspace must look like owner/repo"
        );
    }

    #[test]
    fn cli_error_code_maps_known_error_types() {
        let error = anyhow::Error::from(teamai_core::TeamAIError::user("no home"));

        assert_eq!(cli_error_code(&error), "core_error");
    }

    #[test]
    fn redact_sensitive_text_removes_github_token_like_values() {
        let redacted = redact_sensitive_text(
            "token=ghp_abcdefghijklmnopqrstuvwxyz123456 and github_pat_11ABCDEFG_secret\nGITHUB_TOKEN",
        );

        assert!(!redacted.contains("ghp_"));
        assert!(!redacted.contains("github_pat_"));
        assert!(!redacted.contains("GITHUB_TOKEN"));
        assert_eq!(redacted.matches("[REDACTED]").count(), 3);
    }
}
