use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, SkillLibraryError>;

#[cfg(test)]
static TEST_GITHUB_TOKEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(test)]
static TEST_PROVIDER_TOKENS: std::sync::Mutex<BTreeMap<String, String>> =
    std::sync::Mutex::new(BTreeMap::new());

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
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    #[serde(alias = "github")]
    GitHub,
    #[serde(alias = "gitlab")]
    GitLab,
    Gitee,
    #[serde(alias = "webdav")]
    WebDav,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    PersonalAccessToken,
    DeviceFlow,
    OAuthLoopback,
    Basic,
    AppPassword,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInstance {
    pub id: String,
    pub kind: ProviderKind,
    pub display_name: String,
    pub web_base_url: String,
    pub api_base_url: String,
    #[serde(default)]
    pub auth_modes: Vec<AuthMode>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

pub fn default_provider_instances() -> Vec<ProviderInstance> {
    vec![
        ProviderInstance {
            id: "github.com".to_owned(),
            kind: ProviderKind::GitHub,
            display_name: "GitHub".to_owned(),
            web_base_url: "https://github.com".to_owned(),
            api_base_url: "https://api.github.com".to_owned(),
            auth_modes: vec![AuthMode::PersonalAccessToken, AuthMode::DeviceFlow],
            enabled: true,
        },
        ProviderInstance {
            id: "gitlab.com".to_owned(),
            kind: ProviderKind::GitLab,
            display_name: "GitLab.com".to_owned(),
            web_base_url: "https://gitlab.com".to_owned(),
            api_base_url: "https://gitlab.com/api/v4".to_owned(),
            auth_modes: vec![AuthMode::PersonalAccessToken],
            enabled: true,
        },
        ProviderInstance {
            id: "gitee.com".to_owned(),
            kind: ProviderKind::Gitee,
            display_name: "Gitee".to_owned(),
            web_base_url: "https://gitee.com".to_owned(),
            api_base_url: "https://gitee.com/api/v5".to_owned(),
            auth_modes: vec![AuthMode::PersonalAccessToken],
            enabled: true,
        },
    ]
}

pub fn normalize_provider_id(value: &str) -> String {
    match value {
        "github" => "github.com".to_owned(),
        other => other.to_owned(),
    }
}

fn deserialize_provider_id<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(normalize_provider_id(&value))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRef {
    #[serde(deserialize_with = "deserialize_provider_id")]
    pub provider: String,
    pub owner: String,
    pub repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_id: Option<String>,
}

impl WorkspaceRef {
    pub fn github(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        Self {
            provider: "github.com".to_owned(),
            owner: owner.into(),
            repo: repo.into(),
            remote_id: None,
        }
    }

    pub fn normalized_provider(&self) -> String {
        normalize_provider_id(&self.provider)
    }

    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    pub fn storage_key(&self) -> String {
        format!(
            "{}--{}--{}",
            storage_segment(&self.normalized_provider()),
            storage_segment(&self.owner),
            storage_segment(&self.repo)
        )
    }

    pub fn legacy_storage_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        if self.normalized_provider() == "github.com" && !self.owner.contains('/') {
            keys.push(format!("github.com--{}--{}", self.owner, self.repo));
        }
        keys
    }
}

fn storage_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
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
    #[serde(default)]
    pub provider_instances: Vec<ProviderInstance>,
}

impl Default for SkillLibraryConfig {
    fn default() -> Self {
        Self {
            api_base_url: "http://localhost:8787".to_owned(),
            default_targets: vec!["claude-code".to_owned(), "codex".to_owned()],
            provider_instances: Vec::new(),
        }
    }
}

pub fn configured_provider_instances(config: &SkillLibraryConfig) -> Vec<ProviderInstance> {
    let mut instances: BTreeMap<String, ProviderInstance> = default_provider_instances()
        .into_iter()
        .map(|instance| (normalize_provider_id(&instance.id), instance))
        .collect();
    for instance in &config.provider_instances {
        instances.insert(normalize_provider_id(&instance.id), instance.clone());
    }
    instances.into_values().collect()
}

pub fn read_config(path: impl AsRef<std::path::Path>) -> Result<SkillLibraryConfig> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(SkillLibraryConfig::default());
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(toml::from_str::<SkillLibraryConfig>(&raw)?)
}

pub fn provider_instances_from_config_path(
    path: impl AsRef<std::path::Path>,
) -> Result<Vec<ProviderInstance>> {
    let config = read_config(path)?;
    Ok(configured_provider_instances(&config))
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubCredential {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredentialMetadata {
    pub provider: String,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default = "default_auth_mode")]
    pub auth_mode: AuthMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredential {
    pub metadata: ProviderCredentialMetadata,
    pub token: String,
}

fn default_auth_mode() -> AuthMode {
    AuthMode::PersonalAccessToken
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CredentialsFile {
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderCredentialMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<GitHubCredential>,
}

impl CredentialsFile {
    fn migrate_legacy_github(&mut self) {
        if let Some(github) = &self.github {
            self.providers
                .entry("github.com".to_owned())
                .or_insert_with(|| ProviderCredentialMetadata {
                    provider: "github.com".to_owned(),
                    login: github.login.clone(),
                    scopes: github.scopes.clone(),
                    auth_mode: AuthMode::PersonalAccessToken,
                });
        }
    }
}

pub fn read_credentials(path: impl AsRef<std::path::Path>) -> Result<CredentialsFile> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(CredentialsFile::default());
    }
    let raw = std::fs::read_to_string(path)?;
    let mut credentials: CredentialsFile = serde_json::from_str(&raw)?;
    credentials.migrate_legacy_github();
    Ok(credentials)
}

pub fn write_credentials(
    path: impl AsRef<std::path::Path>,
    credentials: &CredentialsFile,
) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut serializable = credentials.clone();
    serializable.migrate_legacy_github();
    if let Some(github) = serializable.github.as_mut() {
        github.token.clear();
    }
    let bytes = serde_json::to_vec_pretty(&serializable)?;
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

fn legacy_github_keychain_username() -> &'static str {
    "github"
}

fn provider_keychain_username(provider_id: &str) -> String {
    format!("provider:{}", normalize_provider_id(provider_id))
}

fn provider_credential_entry(
    provider_id: &str,
) -> std::result::Result<keyring::Entry, SkillLibraryError> {
    keyring::Entry::new(keychain_service(), &provider_keychain_username(provider_id))
        .map_err(|err| SkillLibraryError::Keychain(err.to_string()))
}

fn legacy_github_credential_entry() -> std::result::Result<keyring::Entry, SkillLibraryError> {
    keyring::Entry::new(keychain_service(), legacy_github_keychain_username())
        .map_err(|err| SkillLibraryError::Keychain(err.to_string()))
}

pub fn read_provider_token(provider_id: &str) -> Result<Option<String>> {
    #[cfg(test)]
    {
        let provider_id = normalize_provider_id(provider_id);
        if provider_id == "github.com" {
            if let Some(token) = TEST_GITHUB_TOKEN.lock().unwrap().clone() {
                return Ok(Some(token));
            }
        }
        Ok(TEST_PROVIDER_TOKENS
            .lock()
            .unwrap()
            .get(&provider_id)
            .cloned())
    }

    #[cfg(not(test))]
    {
        let provider_id = normalize_provider_id(provider_id);
        let entry = provider_credential_entry(&provider_id)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) if provider_id == "github.com" => {
                let legacy = legacy_github_credential_entry()?;
                match legacy.get_password() {
                    Ok(value) => {
                        write_provider_token(&provider_id, &value)?;
                        Ok(Some(value))
                    }
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(err) => Err(SkillLibraryError::Keychain(err.to_string())),
                }
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(SkillLibraryError::Keychain(err.to_string())),
        }
    }
}

pub fn write_provider_token(provider_id: &str, token: &str) -> Result<()> {
    #[cfg(test)]
    {
        let provider_id = normalize_provider_id(provider_id);
        if provider_id == "github.com" {
            *TEST_GITHUB_TOKEN.lock().unwrap() = Some(token.to_owned());
        }
        TEST_PROVIDER_TOKENS
            .lock()
            .unwrap()
            .insert(provider_id, token.to_owned());
        Ok(())
    }

    #[cfg(not(test))]
    {
        let entry = provider_credential_entry(provider_id)?;
        entry
            .set_password(token)
            .map_err(|err| SkillLibraryError::Keychain(err.to_string()))
    }
}

pub fn delete_provider_token(provider_id: &str) -> Result<()> {
    #[cfg(test)]
    {
        let provider_id = normalize_provider_id(provider_id);
        if provider_id == "github.com" {
            *TEST_GITHUB_TOKEN.lock().unwrap() = None;
        }
        TEST_PROVIDER_TOKENS.lock().unwrap().remove(&provider_id);
        Ok(())
    }

    #[cfg(not(test))]
    {
        let entry = provider_credential_entry(provider_id)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(SkillLibraryError::Keychain(err.to_string())),
        }
    }
}

pub fn read_github_token() -> Result<Option<String>> {
    read_provider_token("github.com")
}

pub fn write_github_token(token: &str) -> Result<()> {
    write_provider_token("github.com", token)
}

pub fn delete_github_token() -> Result<()> {
    delete_provider_token("github.com")?;
    #[cfg(not(test))]
    {
        let entry = legacy_github_credential_entry()?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(err) => return Err(SkillLibraryError::Keychain(err.to_string())),
        }
    }
    Ok(())
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
    save_provider_credential(
        path,
        ProviderCredential {
            metadata: ProviderCredentialMetadata {
                provider: "github.com".to_owned(),
                login: credential.login,
                scopes: credential.scopes,
                auth_mode: AuthMode::PersonalAccessToken,
            },
            token: credential.token,
        },
    )
}

pub fn load_github_credential(
    path: impl AsRef<std::path::Path>,
) -> Result<Option<GitHubCredential>> {
    load_provider_credential(path, "github.com").map(|credential| {
        credential.map(|credential| GitHubCredential {
            token: credential.token,
            login: credential.metadata.login,
            scopes: credential.metadata.scopes,
        })
    })
}

pub fn save_provider_credential(
    path: impl AsRef<std::path::Path>,
    mut credential: ProviderCredential,
) -> Result<CredentialsFile> {
    credential.metadata.provider = normalize_provider_id(&credential.metadata.provider);
    let provider_id = credential.metadata.provider.clone();
    let mut credentials = read_credentials(&path)?;
    write_provider_token(&provider_id, &credential.token)?;
    credentials
        .providers
        .insert(provider_id.clone(), credential.metadata.clone());
    if provider_id == "github.com" {
        credentials.github = Some(GitHubCredential {
            token: String::new(),
            login: credential.metadata.login,
            scopes: credential.metadata.scopes,
        });
    }
    write_credentials(path, &credentials)?;
    Ok(credentials)
}

pub fn load_provider_credential(
    path: impl AsRef<std::path::Path>,
    provider_id: &str,
) -> Result<Option<ProviderCredential>> {
    let path = path.as_ref();
    let mut credentials = read_credentials(path)?;
    let provider_id = normalize_provider_id(provider_id);
    let Some(metadata) = credentials.providers.get(&provider_id).cloned() else {
        return Ok(None);
    };
    let token = match read_provider_token(&provider_id)? {
        Some(token) => token,
        None if provider_id == "github.com"
            && credentials
                .github
                .as_ref()
                .is_some_and(|github| !github.token.is_empty()) =>
        {
            let github = credentials.github.as_mut().expect("checked above");
            let token = github.token.clone();
            write_provider_token(&provider_id, &token)?;
            token
        }
        None => return Ok(None),
    };
    if let Some(github) = credentials.github.as_mut() {
        if !github.token.is_empty() {
            github.token.clear();
        }
    }
    if provider_id == "github.com" {
        write_credentials(path, &credentials)?;
    }
    Ok(Some(ProviderCredential { metadata, token }))
}

pub fn delete_github_credential(path: impl AsRef<std::path::Path>) -> Result<()> {
    delete_provider_credential(path, "github.com")
}

pub fn delete_provider_credential(
    path: impl AsRef<std::path::Path>,
    provider_id: &str,
) -> Result<()> {
    let path = path.as_ref();
    let mut credentials = read_credentials(path)?;
    let provider_id = normalize_provider_id(provider_id);
    if provider_id == "github.com" {
        delete_github_token()?;
    } else {
        delete_provider_token(&provider_id)?;
    }
    credentials.providers.remove(&provider_id);
    if provider_id == "github.com" {
        credentials.github = None;
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

    static CREDENTIAL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn workspace_storage_key_is_stable() {
        let workspace = WorkspaceRef::github("acme", "team-skills");
        assert_eq!(workspace.storage_key(), "github.com--acme--team-skills");
    }

    #[test]
    fn default_provider_instances_enable_implemented_adapters() {
        let providers = default_provider_instances();
        let github = providers
            .iter()
            .find(|provider| provider.id == "github.com")
            .unwrap();
        let gitlab = providers
            .iter()
            .find(|provider| provider.id == "gitlab.com")
            .unwrap();
        let gitee = providers
            .iter()
            .find(|provider| provider.id == "gitee.com")
            .unwrap();

        assert!(github.enabled);
        assert!(gitlab.enabled);
        assert!(gitee.enabled);
    }

    #[test]
    fn configured_provider_instances_merge_custom_webdav_instance() {
        let config = SkillLibraryConfig {
            provider_instances: vec![ProviderInstance {
                id: "webdav-company".to_owned(),
                kind: ProviderKind::WebDav,
                display_name: "Company WebDAV".to_owned(),
                web_base_url: "https://dav.example.test/skills".to_owned(),
                api_base_url: "https://dav.example.test/skills".to_owned(),
                auth_modes: vec![AuthMode::Basic, AuthMode::AppPassword],
                enabled: true,
            }],
            ..SkillLibraryConfig::default()
        };

        let providers = configured_provider_instances(&config);

        assert!(providers.iter().any(|provider| provider.id == "github.com"));
        assert!(providers
            .iter()
            .any(|provider| provider.id == "webdav-company"
                && matches!(provider.kind, ProviderKind::WebDav)));
    }

    #[test]
    fn provider_kind_accepts_webdav_alias() {
        let raw = r#"
api_base_url = "http://localhost:8787"
default_targets = ["codex"]

[[provider_instances]]
id = "webdav-company"
kind = "webdav"
displayName = "Company WebDAV"
webBaseUrl = "https://dav.example.test/skills"
apiBaseUrl = "https://dav.example.test/skills"
authModes = ["basic"]
enabled = true
"#;

        let config: SkillLibraryConfig = toml::from_str(raw).unwrap();

        assert!(matches!(
            config.provider_instances[0].kind,
            ProviderKind::WebDav
        ));
    }

    #[test]
    fn provider_kind_accepts_gitlab_and_gitee_aliases() {
        let raw = r#"
api_base_url = "http://localhost:8787"
default_targets = ["codex"]

[[provider_instances]]
id = "gitlab-internal"
kind = "gitlab"
displayName = "Internal GitLab"
webBaseUrl = "https://gitlab.company.test"
apiBaseUrl = "https://gitlab.company.test/api/v4"

[[provider_instances]]
id = "gitee.com"
kind = "gitee"
displayName = "Gitee"
webBaseUrl = "https://gitee.com"
apiBaseUrl = "https://gitee.com/api/v5"
"#;

        let config: SkillLibraryConfig = toml::from_str(raw).unwrap();

        assert!(matches!(
            config.provider_instances[0].kind,
            ProviderKind::GitLab
        ));
        assert!(matches!(
            config.provider_instances[1].kind,
            ProviderKind::Gitee
        ));
    }

    #[test]
    fn workspace_ref_github_constructor_uses_default_instance_id() {
        let workspace = WorkspaceRef::github("acme", "team-skills");
        assert_eq!(workspace.provider, "github.com");
        assert_eq!(workspace.full_name(), "acme/team-skills");
    }

    #[test]
    fn workspace_ref_deserializes_legacy_github_provider() {
        let raw = r#"{"provider":"github","owner":"acme","repo":"team-skills"}"#;
        let workspace: WorkspaceRef = serde_json::from_str(raw).unwrap();
        assert_eq!(workspace.provider, "github.com");
    }

    #[test]
    fn workspace_storage_key_escapes_nested_namespace() {
        let workspace = WorkspaceRef {
            provider: "gitlab-internal".to_owned(),
            owner: "platform/ai".to_owned(),
            repo: "team-skills".to_owned(),
            remote_id: Some("123".to_owned()),
        };
        assert!(!workspace.storage_key().contains('/'));
    }

    #[test]
    fn legacy_github_storage_key_is_available_for_cache_lookup() {
        let workspace = WorkspaceRef::github("acme", "team-skills");
        assert!(workspace
            .legacy_storage_keys()
            .contains(&"github.com--acme--team-skills".to_owned()));
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
        let _guard = CREDENTIAL_TEST_LOCK.lock().unwrap();
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

    #[test]
    fn credentials_read_legacy_github_metadata_into_provider_map() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        std::fs::write(
            &path,
            r#"{"github":{"token":"","login":"octocat","scopes":["repo"]}}"#,
        )
        .unwrap();

        let credentials = read_credentials(&path).unwrap();
        let github = credentials.providers.get("github.com").unwrap();
        assert_eq!(github.login.as_deref(), Some("octocat"));
        assert_eq!(github.scopes, ["repo"]);
    }

    #[test]
    fn load_provider_credential_migrates_legacy_github_file_token() {
        let _guard = CREDENTIAL_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        *TEST_GITHUB_TOKEN.lock().unwrap() = None;
        TEST_PROVIDER_TOKENS.lock().unwrap().clear();
        std::fs::write(
            &path,
            r#"{"github":{"token":"legacy-token","login":"octocat","scopes":["repo"]}}"#,
        )
        .unwrap();

        let credential = load_provider_credential(&path, "github").unwrap().unwrap();
        let credentials = read_credentials(&path).unwrap();

        assert_eq!(credential.token, "legacy-token");
        assert_eq!(credential.metadata.provider, "github.com");
        assert_eq!(
            credentials.github.as_ref().unwrap().login.as_deref(),
            Some("octocat")
        );
        assert_eq!(credentials.github.as_ref().unwrap().token, "");
        assert_eq!(
            read_provider_token("github.com").unwrap().as_deref(),
            Some("legacy-token")
        );
    }

    #[test]
    fn delete_provider_credential_github_clears_legacy_token_and_metadata() {
        let _guard = CREDENTIAL_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        std::fs::write(
            &path,
            r#"{"github":{"token":"","login":"octocat","scopes":["repo"]}}"#,
        )
        .unwrap();

        *TEST_GITHUB_TOKEN.lock().unwrap() = Some("legacy-token".to_owned());
        delete_provider_credential(&path, "github.com").unwrap();

        assert!(load_provider_credential(&path, "github.com")
            .unwrap()
            .is_none());
        let credentials = read_credentials(&path).unwrap();
        assert!(!credentials.providers.contains_key("github.com"));
        assert!(credentials.github.is_none());
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
                providers: BTreeMap::new(),
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
