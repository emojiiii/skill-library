# Provider Adapter Refactor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents are explicitly requested/available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the GitHub-only remote skill workflow into provider instance, credential, factory, and source-provider abstractions while preserving all current GitHub behavior.

**Architecture:** Implement the refactor in two deliverable waves. Wave 1 covers Phase 0-2 from `docs/PROVIDER_ADAPTER_REFACTOR.md`: protect GitHub behavior, add provider instance/credential models, add a GitHub-only factory, and route scan/detail/download paths through generic source traits. GitLab, Gitee, and WebDAV adapters become follow-up plans after the generic GitHub path is proven.

**Tech Stack:** Rust workspace crates, Tauri v2 commands, React/TypeScript wrappers, reqwest, mockito, pnpm, rtk command wrapper.

---

## Scope

This plan intentionally implements Phase 0, Phase 1, and Phase 2 first.

Phase 3-5 adapter crates are not mixed into this first implementation because they would make regressions harder to isolate. After this plan lands, GitLab/Gitee/WebDAV should be added as independent adapter plans using the same `SkillSourceProvider` contract tests.

All local shell commands must be prefixed with `rtk`.

## File Structure

- Modify: `crates/skill-library-core/src/lib.rs`
  - Provider instance configuration types.
  - Generic provider credential metadata/token helpers.
  - `WorkspaceRef` provider-id migration and storage key compatibility.
  - Legacy GitHub credential wrappers remain available.
- Modify: `crates/skill-library-provider/src/lib.rs`
  - Capability enum and expanded capability model.
  - `SourceRef`, `ArchiveDownload`, `ChangeRequest*`.
  - `SkillSourceProvider`, `GitRepositoryProvider`, optional provider extension traits.
  - Keep current `Provider` trait temporarily for compatibility.
- Modify: `crates/skill-library-provider-github/src/lib.rs`
  - Add provider instance aware constructor.
  - Return `github.com` as the provider instance id.
  - Implement `SkillSourceProvider`, `GitRepositoryProvider`, and `ArchiveProvider`.
  - Keep current GitHub-specific methods and old `Provider` impl working.
- Modify: `crates/skill-library-provider-github/src/scan.rs`
  - Keep only GitHub-specific tests or compatibility exports after generic scan moves.
- Create: `crates/skill-library-sync/src/remote_scan.rs`
  - Generic skill scanning/detail logic over `dyn SkillSourceProvider`.
  - Default per-file fallback; optional batch-read hook can be added later.
- Create: `crates/skill-library-sync/src/provider_factory.rs`
  - Resolve `ProviderInstance` + credential into provider trait objects.
  - First implementation supports only `github.com`.
- Modify: `crates/skill-library-sync/src/lib.rs`
  - Add remote/provider generic public entry points.
  - Keep old `scan_github_*`, `read_github_*`, and `add_github_*` wrappers.
  - Route install/review/rollback downloads through `ArchiveProvider` or `SkillSourceProvider::download_snapshot`.
- Modify: `apps/desktop/src-tauri/src/lib.rs`
  - Add generic provider/auth/workspace commands.
  - Keep old GitHub commands as wrappers.
  - Update workspace parsing to support provider instance ids.
- Modify: `crates/skill-library-cli/src/main.rs`
  - Add generic provider auth and remote scan commands.
  - Keep old `login github` compatibility.
- Modify: `apps/desktop/src/lib/skill-library.ts`
  - Add generic TypeScript types/wrappers.
  - Keep old GitHub wrappers.
- Modify lightly: `apps/desktop/src/shell/AuthDialog.tsx`, `apps/desktop/src/shell/RootLayout.tsx`, `apps/desktop/src/widgets/AddWorkspaceDialog.tsx`
  - Surface provider auth list and provider-aware workspace listing without full UI redesign.

---

## Chunk 1: Phase 0 Behavior Protection

### Task 1: Add migration and storage-key tests before implementation

**Files:**
- Modify: `crates/skill-library-core/src/lib.rs`
- Modify: `crates/skill-library-sync/src/lib.rs`

- [ ] **Step 1: Add failing `WorkspaceRef` provider migration tests**

Add tests near existing `workspace_storage_key_is_stable`.

```rust
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
    assert!(workspace.legacy_storage_keys().contains(&"github.com--acme--team-skills".to_owned()));
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run:

```bash
rtk cargo test -p skill-library-core workspace_ref_ -- --nocapture
```

Expected: FAIL because `remote_id`, provider normalization, and legacy helpers do not exist yet.

- [ ] **Step 3: Add failing credential migration tests**

Add tests near `credentials_round_trip_github_token`.

```rust
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
```

- [ ] **Step 4: Run the tests and verify they fail**

Run:

```bash
rtk cargo test -p skill-library-core credentials_read_legacy -- --nocapture
```

Expected: FAIL because `CredentialsFile.providers` does not exist yet.

### Task 2: Add GitHub smoke/contract tests around current provider behavior

**Files:**
- Modify: `crates/skill-library-provider-github/src/lib.rs`
- Modify: `crates/skill-library-sync/src/lib.rs`

- [ ] **Step 1: Add provider id/capability compatibility test**

Add a test that documents the new target behavior.

```rust
#[test]
fn github_provider_reports_default_instance_id() {
    let provider = GitHubProvider::anonymous("https://api.github.com").unwrap();
    assert_eq!(skill_library_provider::SkillSourceProvider::id(&provider), "github.com");
}
```

- [ ] **Step 2: Add sync wrapper compatibility test**

Add a focused test for provider normalization in the old GitHub wrapper path.

```rust
#[tokio::test]
async fn scan_github_wrapper_accepts_legacy_provider_name() {
    let workspace = WorkspaceRef {
        provider: "github".to_owned(),
        owner: "acme".to_owned(),
        repo: "team-skills".to_owned(),
        remote_id: None,
    };

    assert_eq!(workspace.normalized_provider(), "github.com");
}
```

- [ ] **Step 3: Run current focused suites**

Run:

```bash
rtk cargo test -p skill-library-core -p skill-library-provider-github -p skill-library-sync
```

Expected: Existing tests pass before new failing tests are implemented; newly added tests fail until Chunk 2/3.

---

## Chunk 2: Phase 1 Provider Instances And Credentials

### Task 3: Implement provider instance and workspace reference models

**Files:**
- Modify: `crates/skill-library-core/src/lib.rs`

- [ ] **Step 1: Add provider config types**

Add these public types near the credential section.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    GitHub,
    GitLab,
    Gitee,
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
```

- [ ] **Step 2: Add default provider instances**

Implement:

```rust
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
```

- [ ] **Step 3: Update `WorkspaceRef`**

Add `remote_id` and normalization without breaking old serialized data.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRef {
    #[serde(deserialize_with = "deserialize_provider_id")]
    pub provider: String,
    pub owner: String,
    pub repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_id: Option<String>,
}
```

Implement:

```rust
pub fn normalize_provider_id(value: &str) -> String {
    match value {
        "github" => "github.com".to_owned(),
        other => other.to_owned(),
    }
}
```

`WorkspaceRef::github(...)` must set `provider: "github.com"`.

- [ ] **Step 4: Implement storage key escaping and compatibility**

Replace direct `format!("{}.com--{}--{}", ...)` with escaped segments.

```rust
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
```

- [ ] **Step 5: Run core tests**

Run:

```bash
rtk cargo test -p skill-library-core workspace_ -- --nocapture
```

Expected: WorkspaceRef tests pass.

### Task 4: Implement generic credential storage with GitHub wrappers

**Files:**
- Modify: `crates/skill-library-core/src/lib.rs`

- [ ] **Step 1: Add credential metadata types**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredentialMetadata {
    pub provider: String,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub auth_mode: AuthMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredential {
    pub metadata: ProviderCredentialMetadata,
    pub token: String,
}
```

- [ ] **Step 2: Change `CredentialsFile` to include provider map**

Keep `github` for backward-compatible deserialization.

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CredentialsFile {
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderCredentialMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<GitHubCredential>,
}
```

`read_credentials` should migrate `github` metadata into `providers["github.com"]` in memory. `write_credentials` should write the provider map and should not write plaintext tokens.

- [ ] **Step 3: Add generic keychain account helpers**

```rust
fn provider_keychain_username(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

pub fn read_provider_token(provider_id: &str) -> Result<Option<String>>;
pub fn write_provider_token(provider_id: &str, token: &str) -> Result<()>;
pub fn delete_provider_token(provider_id: &str) -> Result<()>;
```

Keep `read_github_token`, `write_github_token`, and `delete_github_token` as wrappers for `github.com`, with fallback read from old account `github`.

- [ ] **Step 4: Add generic credential helpers**

```rust
pub fn save_provider_credential(
    path: impl AsRef<std::path::Path>,
    credential: ProviderCredential,
) -> Result<CredentialsFile>;

pub fn load_provider_credential(
    path: impl AsRef<std::path::Path>,
    provider_id: &str,
) -> Result<Option<ProviderCredential>>;

pub fn delete_provider_credential(
    path: impl AsRef<std::path::Path>,
    provider_id: &str,
) -> Result<()>;
```

Make existing GitHub helpers delegate to these functions.

- [ ] **Step 5: Run credential tests**

Run:

```bash
rtk cargo test -p skill-library-core credentials -- --nocapture
```

Expected: Existing GitHub credential tests and new provider-map migration tests pass.

### Task 5: Add generic provider auth/status commands while keeping old GitHub commands

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `crates/skill-library-cli/src/main.rs`
- Modify: `apps/desktop/src/lib/skill-library.ts`

- [ ] **Step 1: Add Rust auth status response models**

In Tauri command layer, add:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderAuthStatus {
    provider: String,
    display_name: String,
    login: Option<String>,
    scopes: Vec<String>,
    auth_mode: Option<AuthMode>,
    authenticated: bool,
}
```

Keep existing `AuthStatus` fields such as `githubLogin` and `githubScopes` for frontend compatibility, and add `providers: Vec<ProviderAuthStatus>`.

- [ ] **Step 2: Add generic Tauri commands**

Add:

```rust
#[tauri::command]
fn list_provider_instances() -> CommandResult<Vec<ProviderInstance>>;

#[tauri::command]
async fn login_provider_token(provider_id: String, token: String) -> CommandResult<ProviderAuthStatus>;

#[tauri::command]
fn logout_provider(provider_id: String) -> CommandResult<()>;
```

`login_provider_token("github.com", token)` should validate through `GitHubProvider` exactly like `login_github_token` does today.

- [ ] **Step 3: Keep GitHub command wrappers**

Make:

```rust
login_github_token(token) -> login_provider_token("github.com", token)
logout_github() -> logout_provider("github.com")
```

Device flow can remain GitHub-only in this chunk.

- [ ] **Step 4: Add CLI compatibility**

Add:

```text
skill-library login provider <provider-id> --token <token>
skill-library auth logout <provider-id>
```

Keep:

```text
skill-library login github
```

as a wrapper for `github.com`.

- [ ] **Step 5: Add TypeScript wrappers**

In `apps/desktop/src/lib/skill-library.ts`, add:

```ts
export interface ProviderAuthStatus {
  provider: string;
  displayName: string;
  login?: string | null;
  scopes: string[];
  authMode?: string | null;
  authenticated: boolean;
}

export async function listProviderInstances(): Promise<ProviderInstance[]> {
  if (!isTauri) return [];
  return invoke("list_provider_instances");
}

export async function loginProviderToken(providerId: string, token: string): Promise<ProviderAuthStatus> {
  if (!isTauri) return desktopOnly("Login provider");
  return invoke("login_provider_token", { providerId, token });
}
```

- [ ] **Step 6: Run backend and frontend checks**

Run:

```bash
rtk cargo test -p skill-library-core -p skill-library-cli -p skill-library-desktop
rtk pnpm --filter @skill-library/desktop check
```

Expected: old GitHub auth UI and CLI still work; new generic wrappers compile.

---

## Chunk 3: Phase 2 Generic Read And Download Paths

### Task 6: Add source/git/archive trait contracts

**Files:**
- Modify: `crates/skill-library-provider/src/lib.rs`

- [ ] **Step 1: Add granular capability model**

Replace boolean-only fields with capability values, or add a new model if minimizing churn is safer.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Supported,
    Unsupported,
    RequiresConfig,
    Experimental,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub graphql: Capability,
    pub device_flow: Capability,
    pub oauth_loopback: Capability,
    pub personal_access_token: Capability,
    pub repository_archive: Capability,
    pub release_assets: Capability,
    pub change_requests: Capability,
    pub direct_file_write: Capability,
    pub invitations: Capability,
    pub members: Capability,
    pub webhooks: Capability,
    pub discussions: Capability,
    pub file_storage: Capability,
    pub versions_index: Capability,
}
```

Add `ProviderCapabilities::github()` to reduce call-site noise.

- [ ] **Step 2: Add source reference and archive models**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceRef {
    Latest,
    Version(String),
    Git(GitRef),
    Revision(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveDownload {
    pub destination: PathBuf,
    pub extracted_root: PathBuf,
    pub ref_name: String,
    pub bytes: Option<u64>,
}
```

- [ ] **Step 3: Add source and git traits**

```rust
#[async_trait]
pub trait SkillSourceProvider: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;

    async fn list_sources(&self, opts: PageOpts) -> Result<Page<Workspace>>;
    async fn get_source(&self, reference: &WorkspaceRef) -> Result<Workspace>;
    async fn list_files(&self, reference: &WorkspaceRef, at: &SourceRef) -> Result<Vec<FileEntry>>;
    async fn read_file(&self, reference: &WorkspaceRef, at: &SourceRef, path: &str) -> Result<FileBlob>;
    async fn download_snapshot(
        &self,
        reference: &WorkspaceRef,
        at: &SourceRef,
        destination: &Path,
        on_progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<ArchiveDownload>;
}

#[async_trait]
pub trait GitRepositoryProvider: SkillSourceProvider {
    async fn list_tags(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Tag>>;
    async fn list_releases(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Release>>;
    async fn compare_refs(&self, reference: &WorkspaceRef, base: &GitRef, head: &GitRef) -> Result<RefComparison>;
    async fn check_permission(&self, reference: &WorkspaceRef, login: &str) -> Result<PermissionLevel>;
}
```

Keep the existing `Provider` trait until all call sites are migrated.

- [ ] **Step 4: Run provider crate tests**

Run:

```bash
rtk cargo test -p skill-library-provider
```

Expected: compile passes.

### Task 7: Implement generic traits for GitHub

**Files:**
- Modify: `crates/skill-library-provider-github/src/lib.rs`

- [ ] **Step 1: Add instance-aware constructor**

Add an `instance_id` field to `GitHubProvider`.

```rust
pub struct GitHubProvider {
    client: reqwest::Client,
    api_base: String,
    instance_id: String,
    authenticated: bool,
}
```

Add:

```rust
pub fn for_instance(instance: &ProviderInstance, token: Option<String>) -> Result<Self> {
    Self::with_instance_base_url(instance.id.clone(), instance.api_base_url.clone(), token)
}
```

Existing constructors should set `instance_id` to `github.com`.

- [ ] **Step 2: Implement `SkillSourceProvider`**

Map source methods to existing REST methods:

```rust
SourceRef::Latest => default branch from `get_source`
SourceRef::Git(git_ref) => git_ref.value()
SourceRef::Version(version) => tag/version string
SourceRef::Revision(revision) => sha/ref string
```

`download_snapshot` should call `download_tarball_with_progress`.

- [ ] **Step 3: Implement `GitRepositoryProvider`**

Delegate to current list tags/releases/compare/permission logic.

- [ ] **Step 4: Keep old `Provider` impl**

The old `Provider` impl can delegate to the new trait methods to reduce duplication. This keeps existing sync/Tauri code compiling during the migration.

- [ ] **Step 5: Run GitHub mock tests**

Run:

```bash
rtk cargo test -p skill-library-provider-github
```

Expected: existing mockito tests pass; new provider id/capability tests pass.

### Task 8: Move generic remote scan logic into sync

**Files:**
- Create: `crates/skill-library-sync/src/remote_scan.rs`
- Modify: `crates/skill-library-sync/src/lib.rs`
- Modify: `crates/skill-library-provider-github/src/scan.rs`

- [ ] **Step 1: Create `remote_scan.rs`**

Move the generic parts of `skill-library-provider-github/src/scan.rs` into sync:

```rust
pub async fn scan_workspace_skills(
    source: &dyn SkillSourceProvider,
    reference: &WorkspaceRef,
) -> ProviderResult<WorkspaceSkillScan>;

pub async fn scan_workspace_detail(
    source: &dyn SkillSourceProvider,
    reference: &WorkspaceRef,
) -> ProviderResult<WorkspaceDetailScan>;

pub async fn read_skill_detail(
    source: &dyn SkillSourceProvider,
    reference: &WorkspaceRef,
    skill_path: &str,
    ref_name: Option<&str>,
) -> ProviderResult<SkillDetailScan>;
```

Use `SourceRef` instead of `GitRef` at the generic boundary.

- [ ] **Step 2: Add streaming variant over `SkillSourceProvider`**

```rust
pub async fn scan_skill_assets_streaming(
    source: &dyn SkillSourceProvider,
    reference: &WorkspaceRef,
    at: &SourceRef,
    on_batch: impl FnMut(&[SkillAsset]),
) -> ProviderResult<Vec<SkillAsset>>;
```

- [ ] **Step 3: Keep compatibility exports**

If other crates still import `skill_library_provider_github::scan`, leave small wrappers or re-exports temporarily, but sync should no longer import scanning from the GitHub crate.

- [ ] **Step 4: Run sync and GitHub tests**

Run:

```bash
rtk cargo test -p skill-library-sync -p skill-library-provider-github
```

Expected: scan tests pass after import updates.

### Task 9: Add GitHub-only provider factory in sync

**Files:**
- Create: `crates/skill-library-sync/src/provider_factory.rs`
- Modify: `crates/skill-library-sync/src/lib.rs`

- [ ] **Step 1: Add factory result types**

```rust
pub struct ProviderFactory {
    instances: BTreeMap<String, ProviderInstance>,
}

pub struct ProviderHandles {
    pub source: Arc<dyn SkillSourceProvider>,
    pub git: Option<Arc<dyn GitRepositoryProvider>>,
}
```

- [ ] **Step 2: Implement default instance lookup**

`ProviderFactory::default()` should load `default_provider_instances()` into a map.

- [ ] **Step 3: Implement GitHub-only build path**

```rust
pub fn build(
    &self,
    workspace: &WorkspaceRef,
    credential: Option<&ProviderCredential>,
) -> Result<ProviderHandles> {
    match instance.kind {
        ProviderKind::GitHub => {
            let provider = Arc::new(GitHubProvider::for_instance(&instance, credential.map(|c| c.token.clone()))?);
            Ok(ProviderHandles {
                source: provider.clone(),
                git: Some(provider),
            })
        }
        _ => Err(SyncError::ProviderUnsupported(instance.id.clone())),
    }
}
```

- [ ] **Step 4: Add unsupported-provider error**

Add `SyncError::ProviderUnsupported(String)` with a clear message.

- [ ] **Step 5: Run sync tests**

Run:

```bash
rtk cargo test -p skill-library-sync provider_factory -- --nocapture
```

Expected: `github.com` builds; `gitlab.com` returns unsupported until Phase 3.

### Task 10: Add generic sync entry points and keep GitHub wrappers

**Files:**
- Modify: `crates/skill-library-sync/src/lib.rs`

- [ ] **Step 1: Add generic public entry points**

```rust
pub async fn scan_remote_workspace_skills(
    workspace: &WorkspaceRef,
    credential: Option<&ProviderCredential>,
) -> Result<RemoteWorkspaceSkills>;

pub async fn scan_remote_workspace_skills_streaming(
    workspace: &WorkspaceRef,
    credential: Option<&ProviderCredential>,
    on_batch: impl FnMut(&[SkillAsset]),
) -> Result<RemoteWorkspaceSkills>;

pub async fn scan_remote_workspace_detail(
    workspace: &WorkspaceRef,
    credential: Option<&ProviderCredential>,
) -> Result<WorkspaceDetailScan>;

pub async fn read_remote_skill_detail(
    workspace: &WorkspaceRef,
    skill_path: &str,
    ref_name: Option<&str>,
    credential: Option<&ProviderCredential>,
) -> Result<SkillDetailScan>;
```

- [ ] **Step 2: Make old GitHub wrappers delegate**

`scan_github_workspace_skills` should build a temporary `ProviderCredential` for `github.com` when a token is supplied and call `scan_remote_workspace_skills`.

- [ ] **Step 3: Update workspace add wrapper**

`add_github_workspace_with_webhook` should call a new `add_remote_workspace_with_webhook`. Webhook registration should require `git` capability; unsupported providers should return `ProviderUnsupported`.

- [ ] **Step 4: Run compatibility tests**

Run:

```bash
rtk cargo test -p skill-library-sync scan_github -- --nocapture
```

Expected: old function names still pass tests and use the new remote path internally.

### Task 11: Route downloads through generic snapshot/archive APIs

**Files:**
- Modify: `crates/skill-library-sync/src/lib.rs`

- [ ] **Step 1: Refactor `download_skill_source`**

Replace direct `github_provider(...).download_tarball(...)` with:

```rust
let handles = ProviderFactory::default().build(workspace, credential.as_ref())?;
let archive = handles
    .source
    .download_snapshot(workspace, &SourceRef::Git(git_ref.clone()), &cache_dir, &mut |_, _| {})
    .await
    .map_err(sync_provider_error)?;
```

- [ ] **Step 2: Refactor `download_skill_for_install`**

Use `download_snapshot` with progress callback. Preserve the current cache marker behavior.

- [ ] **Step 3: Refactor `prepare_skill_for_review` and `download_review_tarball`**

`download_review_tarball` should accept `&dyn SkillSourceProvider`, not `&GitHubProvider`.

- [ ] **Step 4: Add fallback lookup for legacy cache directories**

When reading a cached workspace path, check `workspace.storage_key()` first, then each `workspace.legacy_storage_keys()`.

- [ ] **Step 5: Run rollback/install/review tests**

Run:

```bash
rtk cargo test -p skill-library-sync rollback_asset download_skill prepare_skill -- --nocapture
```

Expected: existing source override tests still pass; remote download paths compile through generic provider handles.

### Task 12: Add generic Tauri and CLI remote commands

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `crates/skill-library-cli/src/main.rs`
- Modify: `apps/desktop/src/lib/skill-library.ts`

- [ ] **Step 1: Add provider-aware workspace parser**

Implement parser behavior:

- `owner/repo` -> `github.com`, owner `owner`, repo `repo`.
- `github.com/owner/repo` -> provider `github.com`.
- `gitlab-internal/group/subgroup/project` -> provider `gitlab-internal`, owner `group/subgroup`, repo `project`.

The parser should use known provider instance ids to decide whether the first path segment is a provider id.

- [ ] **Step 2: Add generic Tauri workspace commands**

```rust
#[tauri::command]
async fn list_provider_workspaces(provider_id: String, query: Option<String>) -> CommandResult<Vec<Workspace>>;

#[tauri::command]
async fn scan_remote_workspace(workspace: String, token: Option<String>) -> CommandResult<WorkspaceSkillsResponse>;

#[tauri::command]
async fn scan_remote_workspace_streaming(app: tauri::AppHandle, workspace: String, token: Option<String>) -> CommandResult<WorkspaceSkillsResponse>;
```

Keep old GitHub command wrappers.

- [ ] **Step 3: Add generic TypeScript wrappers**

```ts
export async function listProviderWorkspaces(providerId: string, query?: string): Promise<Workspace[]> {
  if (!isTauri) return [];
  return invoke("list_provider_workspaces", { providerId, query });
}

export async function scanRemoteWorkspace(args: { workspace: string; token?: string }): Promise<WorkspaceSkillScan> {
  if (!isTauri) return desktopOnly("Scan remote workspace");
  return invoke("scan_remote_workspace", args);
}
```

- [ ] **Step 4: Keep old wrappers delegating**

`scanGithubWorkspace`, `scanGithubWorkspaceStreaming`, and `listGithubWorkspaces` should call the old commands for now or the new generic commands with `providerId: "github.com"`. Choose the lower-risk option after compile feedback.

- [ ] **Step 5: Add CLI generic remote commands**

Add:

```text
skill-library workspace add <provider-id>/<owner>/<repo>
skill-library scan-remote <provider-id>/<owner>/<repo>
```

Keep old default `owner/repo` as GitHub.

- [ ] **Step 6: Run command-layer checks**

Run:

```bash
rtk cargo test -p skill-library-cli -p skill-library-desktop parse_workspace -- --nocapture
rtk cargo check --workspace
rtk pnpm --filter @skill-library/desktop check
```

Expected: generic commands compile; old GitHub wrappers remain available.

### Task 13: Minimal frontend surfacing for provider-aware auth/workspaces

**Files:**
- Modify: `apps/desktop/src/shell/RootLayout.tsx`
- Modify: `apps/desktop/src/shell/AuthDialog.tsx`
- Modify: `apps/desktop/src/widgets/AddWorkspaceDialog.tsx`
- Modify: `apps/desktop/src/hooks/useLocale.ts`

- [ ] **Step 1: Keep GitHub as the default selected provider**

Add local state in `RootLayout`:

```ts
const [selectedProviderId, setSelectedProviderId] = useState("github.com");
```

Use it for `listProviderWorkspaces(selectedProviderId)` when the generic wrapper is available.

- [ ] **Step 2: Show provider auth statuses in AuthDialog**

Display current provider statuses from `getAuthStatus().providers`. Do not redesign device flow yet; keep GitHub device flow in its existing section.

- [ ] **Step 3: Add provider selector to AddWorkspaceDialog**

Use provider instances from backend. For this chunk, non-GitHub providers can appear disabled or show "coming soon" if the factory returns unsupported.

- [ ] **Step 4: Add locale keys**

Add provider-generic strings in both zh/en sections. Avoid removing existing GitHub strings until Phase 7.

- [ ] **Step 5: Run frontend checks**

Run:

```bash
rtk pnpm --filter @skill-library/desktop check
```

Expected: no TypeScript errors; old GitHub workflow still visible and usable.

---

## Chunk 4: Verification And Handoff

### Task 14: Full verification for Wave 1

**Files:**
- No planned edits.

- [ ] **Step 1: Run Rust formatting**

Run:

```bash
rtk cargo fmt --all
```

Expected: no formatting errors.

- [ ] **Step 2: Run workspace Rust checks**

Run:

```bash
rtk cargo check --workspace
```

Expected: compile passes.

- [ ] **Step 3: Run focused Rust tests**

Run:

```bash
rtk cargo test -p skill-library-core -p skill-library-provider -p skill-library-provider-github -p skill-library-sync -p skill-library-cli
```

Expected: all focused tests pass.

- [ ] **Step 4: Run frontend check**

Run:

```bash
rtk pnpm --filter @skill-library/desktop check
```

Expected: TypeScript/build check passes.

- [ ] **Step 5: Run broader check when time allows**

Run:

```bash
rtk pnpm -r check
rtk cargo test --workspace
```

Expected: full repository checks pass.

### Task 15: Manual smoke checklist

**Files:**
- No planned edits.

- [ ] **Step 1: GitHub token login**

In the desktop app, log in with a GitHub token. Expected: `get_auth_status` includes both legacy `githubLogin` and `providers[{ provider: "github.com", authenticated: true }]`.

- [ ] **Step 2: List GitHub repositories**

Open add workspace. Expected: GitHub repo list still loads.

- [ ] **Step 3: Add and scan workspace**

Add an existing GitHub workspace. Expected: scan, skill list, detail, and file tree behave as before.

- [ ] **Step 4: Install and review download paths**

Install a remote skill and trigger AI review preparation. Expected: download uses the new generic snapshot path without changing user-visible behavior.

- [ ] **Step 5: Unsupported provider behavior**

Try `gitlab.com/group/project` before GitLab adapter exists. Expected: clear unsupported-provider error, not a GitHub-specific error.

---

## Follow-Up Plans After This Lands

### Follow-Up A: GitLab Adapter

- Create `crates/skill-library-provider-gitlab`.
- Add it to `Cargo.toml` workspace members.
- Implement `SkillSourceProvider`, `GitRepositoryProvider`, and archive download for GitLab.com/self-hosted.
- Add mock HTTP tests for nested namespaces, URL-encoded project paths, 401/403/404, pagination, tags, releases, compare, and archive.
- Extend `ProviderFactory` to build GitLab from `ProviderInstance`.

### Follow-Up B: Gitee Adapter

- Create `crates/skill-library-provider-gitee`.
- Implement public Gitee OpenAPI v5 read path first.
- Spike archive endpoint and auth behavior before adding install support.
- Keep enterprise/private Gitee instances configured but capability-gated until verified.

### Follow-Up C: WebDAV Adapter

- Create `crates/skill-library-provider-webdav`.
- Implement `PROPFIND`, `GET`, recursive directory snapshot download, ETag/Last-Modified/content-hash detection.
- Add optional `.skill-library/index.json` support for versions.
- No Git collaboration traits; UI must hide PR/MR, members, invitations, and discussions for WebDAV.

### Follow-Up D: Write Path And Product Polish

- Rename PR-facing models to `ChangeRequest`.
- Move GitHub Discussions behind optional `SocialProvider`.
- Replace remaining GitHub-only locale keys and user-facing errors with provider-aware language.
- Add provider badges and provider-specific external links.
