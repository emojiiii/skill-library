use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, SkillLibraryError>;

#[cfg(test)]
static TEST_GITHUB_TOKEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(test)]
static TEST_AI_KEY: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[derive(Debug, thiserror::Error)]
pub enum SkillLibraryError {
    #[error("{0}")]
    UserMessage(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("toml serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),
    #[error("keychain error: {0}")]
    Keychain(String),
}

impl SkillLibraryError {
    pub fn user(message: impl Into<String>) -> Self {
        Self::UserMessage(message.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeTarget {
    ClaudeCode,
    Cursor,
    Codex,
    Custom(String),
}

impl RuntimeTarget {
    pub fn as_str(&self) -> &str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
            Self::Custom(value) => value.as_str(),
        }
    }

    pub fn from_id(value: &str) -> Self {
        match value {
            "claude-code" => Self::ClaudeCode,
            "cursor" => Self::Cursor,
            "codex" => Self::Codex,
            other => Self::Custom(other.to_owned()),
        }
    }
}

impl fmt::Display for RuntimeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdatePolicy {
    AutoPatch,
    AutoMinor,
    Manual,
    Pin,
}

impl Default for UpdatePolicy {
    fn default() -> Self {
        Self::Manual
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRef {
    pub provider: String,
    pub owner: String,
    pub repo: String,
}

impl WorkspaceRef {
    pub fn github(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        Self {
            provider: "github".to_owned(),
            owner: owner.into(),
            repo: repo.into(),
        }
    }

    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    pub fn storage_key(&self) -> String {
        format!("{}.com--{}--{}", self.provider, self.owner, self.repo)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRef {
    pub workspace: WorkspaceRef,
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLibraryConfig {
    pub api_base_url: String,
    pub default_targets: Vec<String>,
}

impl Default for SkillLibraryConfig {
    fn default() -> Self {
        Self {
            api_base_url: "http://localhost:8787".to_owned(),
            default_targets: vec!["claude-code".to_owned(), "codex".to_owned()],
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub home: PathBuf,
    pub config: PathBuf,
    pub database: PathBuf,
    pub credentials: PathBuf,
    pub subscriptions: PathBuf,
    pub workspace_registry: PathBuf,
    pub workspaces: PathBuf,
    pub staging: PathBuf,
    pub logs: PathBuf,
    pub tmp: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self> {
        let home_dir = dirs::home_dir().ok_or_else(|| {
            SkillLibraryError::user("cannot resolve the current user's home directory")
        })?;
        Ok(Self::for_home(home_dir.join(".skill-library")))
    }

    pub fn for_home(home: PathBuf) -> Self {
        Self {
            config: home.join("config.toml"),
            database: home.join("skill-library.db"),
            credentials: home.join("credentials.json"),
            subscriptions: home.join("subscriptions.yaml"),
            workspace_registry: home.join("workspaces.yaml"),
            workspaces: home.join("workspaces"),
            staging: home.join("staging"),
            logs: home.join("logs"),
            tmp: home.join("tmp"),
            home,
        }
    }

    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.home)?;
        std::fs::create_dir_all(&self.workspaces)?;
        std::fs::create_dir_all(self.staging.join("publish"))?;
        std::fs::create_dir_all(&self.logs)?;
        std::fs::create_dir_all(&self.tmp)?;
        if !self.config.exists() {
            let config = toml::to_string_pretty(&SkillLibraryConfig::default())?;
            std::fs::write(&self.config, config)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CredentialsFile {
    #[serde(default)]
    pub github: Option<GitHubCredential>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubCredential {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub fn read_credentials(path: impl AsRef<std::path::Path>) -> Result<CredentialsFile> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(CredentialsFile::default());
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn write_credentials(
    path: impl AsRef<std::path::Path>,
    credentials: &CredentialsFile,
) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(credentials)?;
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn keychain_service() -> &'static str {
    "Skill Library"
}

fn keychain_username() -> &'static str {
    "github"
}

fn github_credential_entry() -> std::result::Result<keyring::Entry, SkillLibraryError> {
    keyring::Entry::new(keychain_service(), keychain_username())
        .map_err(|err| SkillLibraryError::Keychain(err.to_string()))
}

pub fn read_github_token() -> Result<Option<String>> {
    #[cfg(test)]
    {
        Ok(TEST_GITHUB_TOKEN.lock().unwrap().clone())
    }

    #[cfg(not(test))]
    {
        let entry = github_credential_entry()?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(SkillLibraryError::Keychain(err.to_string())),
        }
    }
}

pub fn write_github_token(token: &str) -> Result<()> {
    #[cfg(test)]
    {
        *TEST_GITHUB_TOKEN.lock().unwrap() = Some(token.to_owned());
        Ok(())
    }

    #[cfg(not(test))]
    {
        let entry = github_credential_entry()?;
        entry
            .set_password(token)
            .map_err(|err| SkillLibraryError::Keychain(err.to_string()))
    }
}

pub fn delete_github_token() -> Result<()> {
    #[cfg(test)]
    {
        *TEST_GITHUB_TOKEN.lock().unwrap() = None;
        Ok(())
    }

    #[cfg(not(test))]
    {
        let entry = github_credential_entry()?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(SkillLibraryError::Keychain(err.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// AI review provider API key — stored in the OS keychain, same mechanism as the
// GitHub token (never written to disk in plaintext).
// ---------------------------------------------------------------------------

fn ai_credential_entry() -> std::result::Result<keyring::Entry, SkillLibraryError> {
    keyring::Entry::new(keychain_service(), "ai-review")
        .map_err(|err| SkillLibraryError::Keychain(err.to_string()))
}

pub fn read_ai_key() -> Result<Option<String>> {
    #[cfg(test)]
    {
        Ok(TEST_AI_KEY.lock().unwrap().clone())
    }

    #[cfg(not(test))]
    {
        let entry = ai_credential_entry()?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(SkillLibraryError::Keychain(err.to_string())),
        }
    }
}

pub fn write_ai_key(key: &str) -> Result<()> {
    #[cfg(test)]
    {
        *TEST_AI_KEY.lock().unwrap() = Some(key.to_owned());
        Ok(())
    }

    #[cfg(not(test))]
    {
        let entry = ai_credential_entry()?;
        entry
            .set_password(key)
            .map_err(|err| SkillLibraryError::Keychain(err.to_string()))
    }
}

pub fn delete_ai_key() -> Result<()> {
    #[cfg(test)]
    {
        *TEST_AI_KEY.lock().unwrap() = None;
        Ok(())
    }

    #[cfg(not(test))]
    {
        let entry = ai_credential_entry()?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(SkillLibraryError::Keychain(err.to_string())),
        }
    }
}

pub fn save_github_credential(
    path: impl AsRef<std::path::Path>,
    credential: GitHubCredential,
) -> Result<CredentialsFile> {
    let mut credentials = read_credentials(&path)?;
    write_github_token(&credential.token)?;
    credentials.github = Some(GitHubCredential {
        token: String::new(),
        login: credential.login,
        scopes: credential.scopes,
    });
    write_credentials(path, &credentials)?;
    Ok(credentials)
}

pub fn load_github_credential(
    path: impl AsRef<std::path::Path>,
) -> Result<Option<GitHubCredential>> {
    let path = path.as_ref();
    let mut credentials = read_credentials(path)?;
    let Some(github) = credentials.github.as_mut() else {
        return Ok(None);
    };
    let token = match read_github_token()? {
        Some(token) => token,
        None if !github.token.is_empty() => {
            let token = github.token.clone();
            write_github_token(&token)?;
            token
        }
        None => return Ok(None),
    };
    let login = github.login.clone();
    let scopes = github.scopes.clone();
    if !github.token.is_empty() {
        github.token.clear();
        write_credentials(path, &credentials)?;
    }
    Ok(Some(GitHubCredential {
        token,
        login,
        scopes,
    }))
}

pub fn delete_github_credential(path: impl AsRef<std::path::Path>) -> Result<()> {
    let path = path.as_ref();
    let mut credentials = read_credentials(path)?;
    delete_github_token()?;
    if let Some(github) = credentials.github.as_mut() {
        github.token.clear();
    }
    write_credentials(path, &credentials)?;
    Ok(())
}

pub fn keychain_store_name() -> &'static str {
    match keyring::default::default_credential_builder().persistence() {
        keyring::credential::CredentialPersistence::UntilDelete => "os-keychain",
        _ => "memory-keyring",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn max(self, other: Self) -> Self {
        use RiskLevel::*;
        match (self, other) {
            (Critical, _) | (_, Critical) => Critical,
            (High, _) | (_, High) => High,
            (Medium, _) | (_, Medium) => Medium,
            _ => Low,
        }
    }

    pub fn requires_confirmation(self) -> bool {
        matches!(
            self,
            RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn permission_risk(permission: &str) -> RiskLevel {
    match permission {
        "secrets.read" => RiskLevel::Critical,
        "filesystem.write" | "shell.execute" | "network.external" => RiskLevel::High,
        "shell.execute.limited" => RiskLevel::Medium,
        _ => RiskLevel::Low,
    }
}

pub fn risk_for_permissions<'a>(permissions: impl IntoIterator<Item = &'a str>) -> RiskLevel {
    permissions
        .into_iter()
        .fold(RiskLevel::Low, |acc, permission| {
            acc.max(permission_risk(permission))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_storage_key_is_stable() {
        let workspace = WorkspaceRef::github("acme", "team-skills");
        assert_eq!(workspace.storage_key(), "github.com--acme--team-skills");
    }

    #[test]
    fn permissions_raise_risk_level() {
        assert_eq!(
            risk_for_permissions(["filesystem.read", "network.external"]),
            RiskLevel::High
        );
    }

    #[test]
    fn credentials_round_trip_github_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        let saved = save_github_credential(
            &path,
            GitHubCredential {
                token: "ghp_secret".to_owned(),
                login: Some("octocat".to_owned()),
                scopes: vec!["repo".to_owned(), "read:org".to_owned()],
            },
        )
        .unwrap();

        assert_eq!(
            saved.github.as_ref().unwrap().login.as_deref(),
            Some("octocat")
        );
        let read = read_credentials(&path).unwrap();
        let github = read.github.unwrap();
        assert_eq!(github.token, "");
        assert_eq!(github.scopes, ["repo", "read:org"]);

        let loaded = load_github_credential(&path).unwrap().unwrap();
        assert_eq!(loaded.token, "ghp_secret");
        assert_eq!(loaded.login.as_deref(), Some("octocat"));
    }

    #[cfg(unix)]
    #[test]
    fn credentials_file_is_owner_only_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        write_credentials(
            &path,
            &CredentialsFile {
                github: Some(GitHubCredential {
                    token: "ghp_secret".to_owned(),
                    login: None,
                    scopes: Vec::new(),
                }),
            },
        )
        .unwrap();

        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
