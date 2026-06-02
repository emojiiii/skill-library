# Provider Adapter 重构方案

> 目标：在保持现有 GitHub 主流程可用的前提下，把 GitHub-only 实现重构为可插拔 Provider Adapter，后续接入 GitLab.com、GitLab 自建内网实例、Gitee 公网，并为企业内网 Gitee / 其他 Git 托管平台预留扩展空间。

## 1. 背景

当前产品定位已经是“Git Provider 上的团队 Skills 工作流层”，但实际实现仍然大量绑定 GitHub：

- `crates/skill-library-provider` 已有 `Provider` trait，这是可复用的抽象基础。
- `crates/skill-library-provider-github` 实现了 GitHub REST / GraphQL、tarball 下载、PR、邀请、Discussions 等能力。
- `crates/skill-library-sync` 中的入口仍是 `scan_github_workspace_skills`、`read_github_skill_detail`、`github_provider(...)`，订阅、下载、回滚、AI review 下载路径都直接构造 `GitHubProvider`。
- `crates/skill-library-core` 的凭据模型只有 `GitHubCredential`，keychain account 也固定为 `github`。
- Tauri command、CLI、前端 wrapper 和 UI 文案大量使用 `github_*` 命名。
- GitHub Discussions 是 GitHub 独有体验，不能作为通用 Provider 必选能力。

因此第二个 Provider 不能靠在命令层继续加 `if provider == "gitlab"`。合理方向是把“Provider 实例配置、认证、能力、仓库读写、发布、邀请、通知”收敛到抽象层，上层只面向能力调用。

## 2. 重构目标

1. GitHub 现有行为不回退：登录、列 workspace、扫描、详情、订阅、下载、同步、diff、发布 PR、邀请、评论能力保持可用。
2. 支持多个 Provider 实例：例如 `github.com`、`gitlab.com`、`gitlab-internal.company.local`、`gitee.com` 可以同时存在。
3. 支持内网 GitLab：用户可以配置 `web_base_url` / `api_base_url`，不假设所有 GitLab 都在公网。
4. 支持 Gitee：优先接入 Gitee OpenAPI v5 公网；企业版 / 私有化实例用同一配置模型预留。
5. 上层按能力降级：Provider 不支持 Discussions、GraphQL、device flow、webhook、邀请时，UI 显示不可用或走轮询/PAT 兜底。
6. 保持数据可迁移：已有 `provider = "github"` 的 workspace、subscription、lockfile 可以无损迁移到新 provider instance。

## 3. 非目标

- 不在这次重构中托管用户代码内容；仓库内容仍从 Provider 按需读取或下载。
- 不实现影子权限系统；权限仍以 Provider 返回结果为准。
- 不强行让所有 Provider 支持 GitHub Discussions 等社交能力。
- 不一次性完成所有写路径。读路径和同步路径应先抽象并落地，发布/邀请/评论后续分阶段迁移。

## 4. 推荐架构

### 4.1 Provider Instance

把“Provider 类型”和“Provider 实例”分开：

```rust
pub enum ProviderKind {
    GitHub,
    GitLab,
    Gitee,
    Custom(String),
}

pub struct ProviderInstance {
    pub id: String,          // github.com, gitlab.com, gitlab-internal, gitee.com
    pub kind: ProviderKind,
    pub display_name: String,
    pub web_base_url: String,
    pub api_base_url: String,
    pub auth_modes: Vec<AuthMode>,
    pub enabled: bool,
}
```

默认实例：

| id | kind | web_base_url | api_base_url |
|---|---|---|---|
| `github.com` | `GitHub` | `https://github.com` | `https://api.github.com` |
| `gitlab.com` | `GitLab` | `https://gitlab.com` | `https://gitlab.com/api/v4` |
| `gitee.com` | `Gitee` | `https://gitee.com` | `https://gitee.com/api/v5` |

自建 GitLab 由用户新增实例：

```json
{
  "id": "gitlab-internal",
  "kind": "gitlab",
  "displayName": "公司 GitLab",
  "webBaseUrl": "https://gitlab.company.local",
  "apiBaseUrl": "https://gitlab.company.local/api/v4"
}
```

### 4.2 WorkspaceRef 升级

当前 `WorkspaceRef { provider, owner, repo }` 对 GitHub 足够，但 GitLab 有 group/subgroup/project，Gitee 有企业/组织/个人空间。建议先做兼容升级：

```rust
pub struct WorkspaceRef {
    pub provider: String,          // provider instance id，不再只是 kind
    pub owner: String,             // namespace，可包含 `/`
    pub repo: String,
    pub remote_id: Option<String>, // GitLab project id / URL-encoded path / Gitee repo id
}
```

约定：

- `provider` 表示实例 id，例如 `github.com`、`gitlab.com`、`gitlab-internal`、`gitee.com`。
- `owner` 表示命名空间，不要求只能是一段。GitLab `group/subgroup/project` 可表达为 `owner = "group/subgroup"`、`repo = "project"`。
- `remote_id` 可选。GitLab API 更适合用 project id 或 URL-encoded full path；保存它可以避免每次拼接和转义。
- `full_name()` 仍返回 `owner/repo`，用于显示。
- `storage_key()` 必须重新实现为稳定 slug，不能继续假设 `provider.com--owner--repo`，因为 `owner` 可能包含 `/`，`provider` 也可能不是域名。

迁移规则：

- 旧数据 `provider = "github"` 迁移为 `provider = "github.com"`。
- 旧的 `github.com--acme--team-skills` storage key 可以继续兼容读取；新写入使用新 key。

### 4.3 Provider Factory

新增 `ProviderRegistry` 或 `ProviderFactory`，统一根据 instance + token 构造 adapter：

```rust
pub trait ProviderFactory: Send + Sync {
    fn build(
        &self,
        instance: &ProviderInstance,
        credential: Option<&ProviderCredential>,
    ) -> skill_library_provider::Result<Box<dyn Provider>>;
}
```

调用方不再直接 `GitHubProvider::new(token)`，而是：

1. 解析 `WorkspaceRef.provider` 得到 instance。
2. 读取该 instance 对应 credential。
3. 通过 factory 创建 `Box<dyn Provider>`。
4. 调用 `Provider` trait 或 optional capability trait。

### 4.4 Provider Trait 分层

现有 `Provider` trait 已覆盖很多读写能力，但 GitHub 实现里还有不少能力在 trait 之外。建议拆成基础能力 + 可选扩展能力：

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;

    async fn current_user(&self) -> Result<ProviderUser>;
    async fn list_workspaces(&self, opts: PageOpts) -> Result<Page<Workspace>>;
    async fn get_workspace(&self, reference: &WorkspaceRef) -> Result<Workspace>;
    async fn list_files(&self, reference: &WorkspaceRef, at: &GitRef) -> Result<Vec<FileEntry>>;
    async fn read_file(&self, reference: &WorkspaceRef, at: &GitRef, path: &str) -> Result<FileBlob>;
    async fn list_tags(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Tag>>;
    async fn list_releases(&self, reference: &WorkspaceRef, opts: PageOpts) -> Result<Page<Release>>;
    async fn compare_refs(&self, reference: &WorkspaceRef, base: &GitRef, head: &GitRef) -> Result<RefComparison>;
    async fn check_permission(&self, reference: &WorkspaceRef, login: &str) -> Result<PermissionLevel>;
}

#[async_trait]
pub trait ArchiveProvider: Send + Sync {
    async fn download_archive(
        &self,
        reference: &WorkspaceRef,
        ref_name: &str,
        destination: &Path,
        on_progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<ArchiveDownload>;
}

#[async_trait]
pub trait PublishProvider: Send + Sync {
    async fn create_change_request(
        &self,
        reference: &WorkspaceRef,
        input: ChangeRequestInput,
    ) -> Result<ChangeRequest>;
}

#[async_trait]
pub trait SocialProvider: Send + Sync {
    async fn list_skill_threads(&self, reference: &WorkspaceRef, skill_ids: &[String]) -> Result<DiscussionStatus>;
}
```

这样 GitLab / Gitee 可以先实现 `Provider + ArchiveProvider + PublishProvider`，暂不实现 `SocialProvider`。

### 4.5 Capability 模型

现有 `ProviderCapabilities` 继续使用，但需要从 bool 变成更细粒度的能力声明：

```rust
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
}

pub enum Capability {
    Supported,
    Unsupported,
    RequiresConfig,
    Experimental,
}
```

UI 和命令层根据能力显示操作，而不是按 Provider 名称分支。

## 5. 认证与凭据

### 5.1 通用 Credential

把 `CredentialsFile.github` 替换为 provider instance map：

```rust
pub struct CredentialsFile {
    pub providers: BTreeMap<String, ProviderCredentialMetadata>,
}

pub struct ProviderCredentialMetadata {
    pub provider: String,
    pub login: Option<String>,
    pub scopes: Vec<String>,
    pub auth_mode: AuthMode,
}

pub struct ProviderCredential {
    pub metadata: ProviderCredentialMetadata,
    pub token: String,
}
```

keychain account 建议：

```text
provider:{instance_id}
provider:{instance_id}:{login}
```

为了兼容旧数据：

- 首次读取时如果发现 `credentials.github`，迁移到 `providers["github.com"]`。
- 如果 keychain 旧 account `github` 存在，把 token 复制到 `provider:github.com`，再清理旧 metadata 中的明文 token。

### 5.2 Auth Mode

| Provider | 推荐第一阶段 | 说明 |
|---|---|---|
| GitHub | PAT + device flow | 保持现有能力 |
| GitLab.com | PAT 优先，OAuth loopback 后续 | PAT 容易先跑通；OAuth 需要 client 配置 |
| GitLab 自建 | PAT 优先 | 内网实例未必配置 OAuth app/device flow |
| Gitee | PAT / access token 优先 | 先使用 token，OAuth 后续 spike |

上层命令改成：

```text
login_provider_token(provider_id, token)
logout_provider(provider_id)
get_auth_status() -> Vec<ProviderAuthStatus>
```

保留旧命令 `login_github_token`、`logout_github` 作为兼容 wrapper。

## 6. 代码改造范围

### 6.1 crates/skill-library-provider

新增或调整：

- `ProviderKind`
- `ProviderInstance`
- `ProviderCredential`
- `ProviderUser`
- `ArchiveProvider`
- `ArchiveDownload`
- `ChangeRequestInput`
- `ChangeRequest`
- capability 分层
- provider contract test helper

注意：不要把 GitLab/Gitee 字段塞进通用模型。Provider 特有字段保留在 adapter 内部，只有上层确实需要展示时再以 `metadata: serde_json::Value` 暴露。

### 6.2 crates/skill-library-provider-github

目标是“实现通用接口，不再被 sync/Tauri/CLI 直接特殊对待”：

- `GitHubProvider::with_base_url(...)` 继续保留，服务 GitHub Enterprise 兼容。
- `download_tarball_with_progress` 挂到 `ArchiveProvider`。
- `publish_files_pull_request` 改为实现 `PublishProvider::create_change_request`。
- Discussions 相关能力挂到 `SocialProvider`，不进入基础 `Provider`。
- GraphQL 批量取 manifest 是 GitHub adapter 内部优化；上层不感知。

### 6.3 新增 crates/skill-library-provider-gitlab

第一阶段建议只做读路径 + archive：

- list workspaces：列当前用户可访问 projects。
- get workspace：读取 project metadata、default branch、visibility、当前用户权限。
- list files：repository tree。
- read file：repository files 或 raw file。
- list tags / releases。
- compare refs。
- download archive。

GitLab 重点差异：

- project path、branch、tag、file path 中的 `/` 需要 URL encode。
- 自建实例的 base URL 由用户配置，不应写死 `gitlab.com`。
- Merge Request 对应 GitHub PR，通用模型命名应使用 `ChangeRequest`。
- webhook secret / signing 行为与 GitHub 不同，不能复用 GitHub HMAC 假设。

### 6.4 新增 crates/skill-library-provider-gitee

第一阶段建议只做读路径 + archive：

- list workspaces：`/user/repos`、组织/企业 repo 后续补齐。
- get workspace：仓库详情。
- list files / read file：contents API。
- list tags / releases。
- compare refs。
- list members / permission。
- download archive：需要 spike 确认最稳 endpoint 和鉴权方式。

Gitee 重点差异：

- Gitee 是国内公网平台，不能和 Gitea/Forgejo 混为一类。
- Gitee OpenAPI v5 与 GitHub 有相似 endpoint，但字段、分页、鉴权参数、企业能力可能不同。
- Gitee 企业版 / 私有化可能使用不同 API 版本，应通过 `ProviderInstance` 配置预留。

### 6.5 crates/skill-library-sync

这是最关键的去 GitHub 化模块。

重命名入口：

| 当前 | 新名称 |
|---|---|
| `scan_github_workspace_skills` | `scan_remote_workspace_skills` |
| `scan_github_workspace_skills_streaming` | `scan_remote_workspace_skills_streaming` |
| `scan_github_workspace_detail` | `scan_remote_workspace_detail` |
| `read_github_skill_detail` | `read_remote_skill_detail` |
| `add_github_workspace_with_webhook` | `add_remote_workspace_with_webhook` |
| `github_provider(...)` | `provider_for_workspace(...)` |

同时把 `scan.rs` 从 `provider: &GitHubProvider` 改为 `provider: &dyn Provider` 或泛型 `P: Provider + ?Sized`。如果需要批量读取优化，额外定义：

```rust
pub trait BatchReadProvider {
    async fn batch_read_text_files(...);
}
```

没有批量能力时，默认 fallback 为多次 `read_file`。

下载路径必须依赖 `ArchiveProvider`，不能再调用 `GitHubProvider::download_tarball`：

- `download_skill_source`
- `download_skill_for_install`
- `prepare_skill_for_review`
- `download_review_tarball`
- rollback / sync 自动更新

### 6.6 apps/desktop/src-tauri

新增通用命令：

- `list_provider_instances`
- `upsert_provider_instance`
- `delete_provider_instance`
- `login_provider_token`
- `logout_provider`
- `list_provider_workspaces`
- `scan_remote_workspace`
- `scan_remote_workspace_streaming`
- `get_workspace_detail`
- `get_skill_detail`
- `invite_collaborator`
- `create_change_request`

保留旧命令作为 wrapper：

- `login_github_token` 调 `login_provider_token("github.com", token)`。
- `scan_github_workspace` 调 `scan_remote_workspace`。
- `list_github_workspaces` 调 `list_provider_workspaces("github.com")`。

这样前端可以分阶段迁移，不需要一次性改完所有页面。

### 6.7 crates/skill-library-cli

CLI 应从 provider 参数开始泛化：

```text
skill-library login provider <provider-id> --token ...
skill-library auth logout <provider-id>
skill-library workspace add <provider-id>/<owner>/<repo>
skill-library scan-remote <provider-id>/<owner>/<repo>
skill-library invite <provider-id>/<owner>/<repo> <login>
```

兼容旧命令：

```text
skill-library login github
```

内部解析为 `provider-id = github.com`。

### 6.8 前端

前端分两层处理：

1. `apps/desktop/src/lib/skill-library.ts` 先增加通用 wrapper，旧 GitHub wrapper 继续导出。
2. 页面和文案逐步从 GitHub-only 改为 Provider-aware。

需要重点改的 UI：

- Settings/AuthDialog：显示多个 Provider 账户。
- WorkspacePicker/AddWorkspaceDialog：先选 Provider，再列仓库。
- Discover/Subscriptions/MySkills：workspace 展示 provider badge。
- Publish/Invitations/Activity：GitHub PR / GitHub invitation 文案改成 Change Request / Provider invitation。
- SkillComments：仅当 `capabilities.discussions == Supported` 时展示；否则隐藏或显示 Provider 不支持。

## 7. Provider 差异矩阵

| 能力 | GitHub | GitLab.com / 自建 | Gitee |
|---|---|---|---|
| 公网默认实例 | 支持 | 支持 | 支持 |
| 内网实例 | GitHub Enterprise 可预留 | 必须支持 | 企业版预留 |
| 认证第一阶段 | PAT + device flow | PAT | access token / PAT |
| 仓库列表 | 支持 | projects | user/org/enterprise repos |
| 文件树 | REST tree + GraphQL 优化 | repository tree | contents / tree 需 spike |
| 单文件读取 | contents/blob | repository files/raw | contents |
| tag/release | 支持 | 支持 | 支持 |
| archive 下载 | tarball/codeload | repository archive 需实现 | 需 spike |
| PR/MR | Pull Request | Merge Request | Pull Request |
| 邀请/成员 | collaborators/org/team | members/invitations 差异较大 | collaborators/企业能力差异 |
| webhook | HMAC header | token/signing 行为不同 | webhook 类型和签名需 spike |
| Discussions/评论社区 | GitHub Discussions | 无等价通用能力 | 无等价通用能力 |

## 8. 分阶段落地计划

### Phase 0：现状保护

- 为现有 GitHub 主流程补 contract / smoke tests。
- 固化旧数据迁移样例：workspace registry、subscriptions、lockfile、credentials。
- 确认 `rtk` 本地 wrapper 可用，否则 CI 命令只能在无 wrapper 环境下执行。

验收：

- GitHub 登录、列 repo、扫描、详情、下载、同步、diff、发布 PR、邀请仍通过。

### Phase 1：Provider 配置和凭据抽象

- 新增 `ProviderInstance`、`ProviderCredential`、credential map。
- 增加默认 provider instances。
- 迁移 GitHub credential。
- 新增 `ProviderFactory`，但 factory 先只返回 GitHub adapter。
- Tauri/CLI 新增通用 auth 命令，旧 GitHub 命令保留。

验收：

- 旧 GitHub 登录状态自动迁移。
- `github.com` 作为 provider instance 工作正常。
- UI 可显示 provider account list。

### Phase 2：读路径去 GitHub 化

- `scan.rs` 改为依赖 `Provider` trait。
- `skill-library-sync` 入口重命名为 remote/provider 通用名称。
- `download_skill_source`、`download_skill_for_install`、`prepare_skill_for_review` 依赖 `ArchiveProvider`。
- Tauri/CLI 扫描和详情命令改走通用入口。

验收：

- GitHub 读路径行为不变。
- sync crate 不再直接构造 `GitHubProvider`。
- 没有实现 `ArchiveProvider` 的 adapter 会返回明确 capability error。

### Phase 3：GitLab adapter

- 新增 `skill-library-provider-gitlab`。
- 支持 GitLab.com 和自建 base URL。
- 实现基础读能力、权限检查、archive 下载。
- 实现 MR 创建前的最小写路径 spike。
- 加入 mock HTTP contract tests。

验收：

- 能添加 GitLab.com public/private repo workspace。
- 能添加自建 GitLab workspace。
- 能扫描 Skill、查看详情、下载并安装。
- 对无权限、token 失效、URL encode 路径返回统一 `ProviderError`。

### Phase 4：Gitee adapter

- 新增 `skill-library-provider-gitee`。
- 支持 Gitee 公网默认实例。
- 实现基础读能力、权限检查、release/tag、compare。
- 完成 archive 下载 spike。
- 企业版/私有化实例只保留配置能力，除非实测 API 后再开启。

验收：

- 能添加 Gitee repo workspace。
- 能扫描 Skill、查看详情、下载并安装。
- Gitee 不支持或未实现的能力在 UI 中正确隐藏/降级。

### Phase 5：写路径泛化

- 把 publish PR 改成 `ChangeRequest`。
- GitHub 使用 Pull Request，GitLab 使用 Merge Request，Gitee 使用 Pull Request。
- 邀请/成员能力按 Provider capability 展示。
- Activity/Notifications/Webhook 改为 provider-aware。

验收：

- GitHub 发布行为不回退。
- GitLab/Gitee 至少能创建 change request 或明确显示“不支持/未配置”。
- 权限校验仍使用当前用户 token，不通过 bot 提权。

### Phase 6：社交能力和产品打磨

- GitHub Discussions 保持为 GitHub-only optional feature。
- GitLab/Gitee 可考虑 issue/comment 替代，但必须作为独立 capability，不污染基础 Provider。
- UI 文案从 GitHub-only 改为 Provider-aware。

验收：

- 非 GitHub workspace 不出现“GitHub Discussions 未开启”这类错误文案。
- Provider badge、外链、错误提示都显示具体平台。

## 9. 测试策略

1. Provider contract tests：对每个 adapter 跑同一组 trait 行为测试。
2. Mock HTTP tests：覆盖分页、401/403/404、rate limit、path encode、archive 下载失败。
3. 迁移测试：旧 `credentials.github`、旧 `provider = "github"`、旧 storage key。
4. CLI smoke：`login`、`workspace add`、`scan-remote`、`sync`。
5. Desktop command tests：通用 command 和 GitHub wrapper 都要保留测试。
6. UI smoke：Provider 选择、登录状态、workspace picker、详情页、安装流程。

## 10. 风险与处理

| 风险 | 处理 |
|---|---|
| GitLab nested namespace 打破 `owner/repo` 假设 | `owner` 允许包含 `/`，保存 `remote_id` |
| 自建 GitLab 证书/代理/内网不可达 | ProviderInstance 增加 timeout/proxy/tls 配置预留，错误统一 `NetworkError` |
| Gitee API v5/企业版 API 差异 | 公网 v5 先落地，企业版作为单独 instance + capability spike |
| 不同 Provider archive 解压根目录不同 | `ArchiveDownload` 只返回 `extracted_root`，adapter 内部处理 |
| webhook 签名差异 | 不在基础 trait 里假设 HMAC；每个 adapter 自己 verify |
| UI 文案仍写死 GitHub | Phase 6 做 locale 全量扫描和替换 |
| 发布写路径权限复杂 | 读路径先落地；写路径以 capability + provider-specific spike 推进 |

## 11. 建议优先级

推荐先做 Phase 1 + Phase 2。原因是当前项目已经有 Provider trait，但调用层绕过了它；只要把 factory、credential、sync 入口理顺，GitLab/Gitee 接入就变成“新增 adapter + contract tests”，而不是在 Tauri/CLI/前端各处继续复制 GitHub 分支。

第二优先级是 GitLab adapter，因为内网诉求最强，且 GitLab 自建实例要求架构真正支持可配置 base URL。Gitee 放在 GitLab 后，能复用已经验证过的 provider instance、credential、archive、capability 降级机制。

## 12. 外部参考

- GitLab REST API: https://docs.gitlab.com/api/rest/
- GitLab Repositories API: https://docs.gitlab.com/api/repositories/
- GitLab Project Webhooks API: https://docs.gitlab.com/api/project_webhooks/
- Gitee OpenAPI v5: https://gitee.com/api/v5/swagger
- Gitee OpenAPI v5 SDK / Repositories API: https://gitee.com/sdk
- Gitee WebHook 帮助中心: https://help.gitee.com
