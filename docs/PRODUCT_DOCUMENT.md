# Team AI Hub 产品文档 v2.1

## 1. 产品概述

Team AI Hub 是一个 **基于 Git Provider 之上的团队 AI Skills 工作流层**。

团队把 Skills 放在 Git 仓库里（GitHub 优先，其他 Provider 后续支持），Team AI Hub 提供：

- **可视化浏览与管理** —— Web UI 让非工程师也能看懂、搜索、订阅 Skills。
- **声明式订阅 + 自动更新** —— 个人或团队订阅 Skills，本地客户端按策略自动拉取与同步。
- **跨 IDE / Agent 安装** —— Rust 原生 installer 把一份 Skill 安装到 Claude Code、Cursor、Codex 等 runtime。
- **版本回溯与对比** —— 任意两个版本一键 diff，一键回滚。
- **个人到团队的发布流** —— 本地个人 Skill 可以一键发 PR 同步到团队 Workspace，由 bot 按策略自动合并或等待 review。
- **邀请与 onboarding** —— 在 Team AI Hub 里邀请同事，底层仍然落到 GitHub / GitLab / Gitea 的成员关系和仓库权限。
- **团队空间** —— 一个仓库就是一个团队空间，权限、审核、审计直接复用 Git Provider 原生能力。

我们不重新发明用户系统、不重新发明权限、不重新发明 PR / Review、不重新发明审计 —— 这些 Git Provider 已经做得很好。我们只做 AI Skills 场景下，它们没解决的部分：订阅、同步、发布、风险提示、邀请体验与跨 agent 的团队分发。

## 2. 一句话定位

> **Vercel 之于代码仓库的关系，就是 Team AI Hub 之于 AI 资产仓库的关系。**

底层是 Git Provider（GitHub / GitLab / Gitea / 自建 Git），上层是面向 AI 资产的体验层、协议层、分发层。

## 3. 设计原则

### 3.1 不与 Git Provider 竞争，构建在它之上

Git Provider 已经免费提供：仓库存储、用户系统、团队权限、分支保护、PR / Review、commit 历史、tag、release、webhook、审计日志。我们全部复用，不重做。

### 3.2 Provider 可插拔

第一版只支持 GitHub。但从架构第一天起，"Git Provider" 就是一个接口，未来可加 GitLab、Gitea、Bitbucket、企业自建 Git。中国市场、企业内网用户从一开始就在路线图里。

### 3.3 一个仓库 = 一个团队空间

不发明新的 "Workspace" 概念。一个 GitHub repo 就是一个团队空间，repo 的 collaborators / teams 就是空间成员，repo 的权限就是空间权限。

### 3.4 Skills First，其他资产是文件

第一个适配的资产类型是 Skills，因为它有相对成熟的安装/更新协议。其他资产（Prompt、Workflow JSON、Knowledge 文档等）本质都是带元数据的文件托管 —— 等 Skills 跑通后再以扩展形式接入，不影响核心架构。

### 3.5 参考生态工具，原生实现最小 installer

`skills` CLI 已经覆盖 Claude Code、Cursor、Codex 等 agent 的安装路径、全局/项目级 scope、add/list/update/remove/init 等基础能力，是重要参考实现。但 Team AI Hub 桌面端不内置 Node / Bun，MVP 用 Rust 实现自己需要的最小 installer，把工程重心放在团队订阅、发布 PR、权限校验、风险策略和可视化治理上。

### 3.6 本地优先

订阅的 Skills 缓存在本地，离线可用。云端只做协作和分发，不绑架数据。

### 3.7 非工程师也是用户

工程师可以 `git clone` + 手写脚本搞定一切，但 PM、设计、客服、运营不会。Web UI 是把可触达用户从 10% 扩大到 100% 的关键。

### 3.8 邀请体验可以在我们这里，成员关系必须在 Provider 那里

Team AI Hub 可以提供"邀请同事加入团队"的入口、邀请收件箱、接受邀请后的 onboarding 页面，但最终成员关系必须落到 GitHub / GitLab / Gitea / GitHub Enterprise 等 Provider。我们不维护影子成员表，不让 bot 成为权限绕过通道。

## 4. 目标用户

### 4.1 个人开发者
- 把自己的 Skills 放在个人 repo 里。
- 在多台设备间自动同步。
- 跨 Claude Code、Cursor、Codex 一致使用。
- 订阅团队或社区的 Skills。

### 4.2 团队成员
- 在 Web UI 浏览团队 repo 里的 Skills。
- 一键订阅，自动安装到本地 IDE。
- 收到更新通知。
- 接受团队邀请后完成 GitHub 登录 / 注册引导，自动看到自己有权限的 Workspace。
- 包括非工程师角色。

### 4.3 Skill 作者 / 维护者
- 像维护代码一样维护 Skills（commit、tag、PR、release）。
- 用 Web UI 看订阅数、版本分布、使用反馈。
- 通过 Git Provider 原生 PR 流程接受贡献。
- 从本地个人 Skills 一键 publish 到团队 Workspace，生成标准 PR。

### 4.4 团队管理员
- 直接用 GitHub teams / branch protection 管理权限。
- 在 Team AI Hub 里邀请成员，本质是调用 Provider 的 org/team/repo collaborator 邀请 API。
- 在 Web UI 看到团队订阅情况、Skill 健康度。
- 不需要再学一套新的权限模型。

## 5. 核心概念

### 5.1 Workspace（团队空间）

一个 Workspace 就是一个 Git 仓库。

```text
github.com/acme/team-skills           → "Acme 团队空间"
github.com/alice/personal-skills      → "Alice 的个人空间"
github.com/awesome/community-skills   → "社区共享空间"
```

Workspace 的属性都来自底层 repo：

| Workspace 属性 | 来源 |
|---|---|
| 名称 | repo 名 |
| 成员 | repo collaborators / org teams |
| 权限 | repo permission (read / write / admin) |
| 邀请 | org/team/repo collaborator invitation |
| 历史 | git log |
| 审核流 | Pull Request |
| 审计 | GitHub audit log API |

### 5.2 Asset（资产）

仓库里的一个目录就是一个 Asset。MVP 只识别 Skill 类型。

```text
team-skills/
├── code-reviewer/           ← 一个 Skill 资产
│   ├── SKILL.md
│   ├── manifest.yaml
│   └── scripts/
├── pr-summarizer/           ← 另一个 Skill 资产
│   └── SKILL.md
└── README.md
```

每个 Asset 目录下有一个 `manifest.yaml`（或 `SKILL.md` 的 frontmatter）描述元数据。

### 5.3 Manifest（资产元数据）

最小可用的 manifest：

```yaml
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews code changes for correctness and security.
version: 1.4.2
targets:
  - claude-code
  - cursor
  - codex
permissions:
  - filesystem.read
  - shell.execute.limited
```

`version` 字段使用 SemVer，与 git tag 对齐（`v1.4.2`）。

### 5.4 Subscription（订阅）

订阅声明 "我想用哪些 Skills，按什么策略更新"。订阅可以是个人级或团队级。

```yaml
# ~/.team-ai-hub/subscriptions.yaml
subscriptions:
  - workspace: github.com/acme/team-skills
    assets:
      - id: code-reviewer
        channel: stable
        update: auto-patch
      - id: pr-summarizer
        version: ^2.0.0
        update: manual
    targets:
      claude-code: enabled
      cursor: enabled
      codex: disabled
```

### 5.5 Sync（同步）

本地客户端定期或按需拉取订阅的资产，安装/更新到对应 runtime。

```text
本地客户端启动
→ 读取 subscriptions.yaml
→ 通过 Provider API 拉取每个 workspace 的最新 manifest
→ 对比本地 lockfile，决定要更新的资产
→ 下载新版本（git archive 或 release tarball）
→ 验证完整性
→ 通过 `skills` CLI 安装到 Claude Code / Cursor / Codex 的 skills 目录
→ 更新 lockfile
```

### 5.6 Publish（个人 Skill 发布到团队）

发布声明 "把我本地的个人 Skill 同步到某个团队 Workspace"。底层不是直接写主分支，而是由 Team AI Hub Bot 代表用户创建 branch + PR：

```text
本地个人 Skill
→ teamai publish code-reviewer --workspace acme/team-skills
→ 校验发起人对目标 repo 至少有 write 权限
→ 打包 Skill，计算 hash，生成 manifest / risk summary
→ Bot 创建 branch: teamai/import/code-reviewer/<short-sha>
→ Bot 创建 PR，PR body 写明来源人、来源路径、来源 hash、风险等级
→ CI / policy check 通过
→ low risk + trusted user 可 auto-merge；否则等待人工 review
```

核心原则：

> Team AI Hub Bot 只代表用户执行 Git 操作，不提升用户权限。

### 5.7 Invite（邀请）

邀请是 Provider 成员关系的 UI 包装：

| Workspace 类型 | 邀请方式 |
|---|---|
| 个人账号下 private repo | 邀请 repo collaborator |
| GitHub Organization repo | 邀请 org member，并加入对应 team |
| public repo | 不需要邀请即可浏览/安装公开 Skill，publish 走外部贡献流程 |
| GitHub Enterprise / GitLab / Gitea 内网 | 自托管 Team AI Hub 调用对应 Provider 的成员 API |

如果被邀请人还没有 GitHub 账号，Team AI Hub 可以展示注册引导并把用户带到 GitHub 注册；注册完成后再回到 Team AI Hub 完成 Provider 邀请接受。我们不直接创建 GitHub 账号。

## 6. Provider 抽象

### 6.1 接口定义

所有 Provider 实现以下能力：

```text
Provider interface:
  - 列出用户可访问的 workspaces (repos)
  - 读取 workspace 文件树
  - 读取文件内容（任意 ref）
  - 列出 tags / releases
  - 列出 commits / 比较两个 ref 的 diff
  - 读取成员列表与角色
  - 发起 / 查询 / 接受成员或 repo collaborator 邀请
  - 注册 webhook 用于 push 事件通知
  - 通过 OAuth 进行用户身份验证
```

### 6.2 第一版：GitHub

- 用户登录可先用 OAuth / Device Flow 跑通；团队 bot 与邀请能力优先评估 GitHub App。
- GitHub App 安装到 org/repo 后，用细粒度权限处理 repo 访问、PR 创建、webhook、成员邀请。
- 通过 GitHub REST + GraphQL API 拉取 manifest 和文件内容。
- 通过 webhook 接收 push 事件，触发订阅者的更新检查。
- Release tarball 作为资产分发载体（避免每次都打包整个仓库）。

### 6.3 Bot 权限原则

Team AI Hub Bot 可以拥有 repo 写权限或成员邀请权限，但每次执行前必须用发起人的 Provider 身份做实时权限校验：

| 操作 | 发起人最低权限 | Bot 执行动作 |
|---|---|---|
| 浏览 / 安装团队 Skill | read | 不需要写权限 |
| publish 本地 Skill 到团队 repo | write | 创建 branch + PR |
| auto-merge publish PR | maintain/admin 或 branch protection 允许 | merge PR |
| 邀请 repo collaborator | repo admin / owner | 调 Provider invitation API |
| 邀请 org member + 加 team | org owner / team maintainer（视 Provider 规则） | 创建 org/team invitation |

### 6.4 路线图中的 Provider

- GitLab（含自建）
- Gitea / Forgejo（开源自托管 Git，国内常用）
- Bitbucket
- Azure DevOps
- 通用 Git over SSH（兜底方案）

每个 Provider 实现完整接口即可接入，不需要改核心。

### 6.5 Provider 不支持时的兜底

如果某个能力 Provider 没有（例如某些自建 Git 没有 webhook 或成员邀请 API），降级为定时轮询或展示"请到 Provider 管理成员"的引导。这是 Provider 适配层内部处理，对上层透明。

## 7. 资产模型

### 7.1 MVP 只识别 Skill

仓库根目录下每个含 `SKILL.md` 或 `manifest.yaml` 的目录被识别为一个 Skill。其他文件不影响（README、scripts、文档自由放置）。

### 7.2 Skill 包结构

```text
code-reviewer/
├── SKILL.md            ← 必需，主体内容（指令 + 元数据 frontmatter）
├── manifest.yaml       ← 可选，独立元数据文件
├── scripts/            ← 可选，可执行脚本
├── templates/          ← 可选，模板文件
├── examples/           ← 可选，使用示例
└── CHANGELOG.md        ← 可选，变更记录
```

### 7.3 未来扩展的资产类型

后续以扩展形式支持，不影响核心架构：

- **Prompt**：单个 markdown 或 yaml 文件 + frontmatter 元数据。
- **Workflow**：一个 JSON 文件 + 元数据，按特定 schema 解析。
- **Knowledge Pack**：一组文档 + 索引描述。
- **其他**：媒体、数据集、文档 —— 在底层都只是 "带 manifest 的文件"，复用同一套订阅、同步、版本机制。

这些不在 MVP 范围，列在路线图里。

## 8. 版本与回溯

### 8.1 版本来源

直接复用 git：

- **版本号**：git tag（推荐 `v1.4.2` 格式，符合 SemVer）。
- **历史**：git log。
- **changelog**：仓库里的 CHANGELOG.md，或 GitHub Release notes。
- **稳定性通道**：用 git branch / release 的 prerelease 标志区分 stable / beta。

### 8.2 版本对比

Web UI 提供两种 diff 视图：

- **文件 diff**：基于 git 的标准三路 diff，关键字段（如 `permissions`、`version`、`targets`）高亮。
- **语义 diff**：解析 manifest，结构化展示新增/删除/修改的能力、权限、依赖。

可以选中任意两个 tag 一键对比。

### 8.3 回滚

回滚不修改远程仓库，只修改本地 lockfile + 重装本地资产到指定旧版本。如果用户希望整个团队回滚，操作仓库本身（revert commit / move tag），下次 sync 自动生效。

## 9. 订阅与自动更新

### 9.1 订阅声明

订阅文件支持两种存放位置：

- **个人订阅**：`~/.team-ai-hub/subscriptions.yaml`
- **团队订阅**：仓库内 `.teamaihub/subscriptions.yaml`，团队所有成员共享

### 9.2 更新策略

每个订阅项独立配置：

```yaml
update: auto-patch     # 自动更新 patch 版本（1.4.x）
update: auto-minor     # 自动更新到下个 minor（1.x.x）
update: manual         # 仅手动更新
update: pin            # 锁定当前版本
channel: stable        # 跟随 stable 通道
channel: beta          # 跟随 beta 通道
```

### 9.3 更新触发

三种触发方式：

- **Webhook 推送**：Provider push 事件 → 通知本地客户端检查更新。
- **定时拉取**：客户端按配置间隔（默认 1 小时）轮询。
- **手动**：CLI `teamai sync` 或 Web UI 点击。

### 9.4 更新失败处理

- 下载失败 → 重试，保留旧版本可用。
- 安装失败 → 自动回滚到上一个 working 版本。
- 元数据损坏 → 跳过该资产，不影响其他订阅。

## 10. 同步与本地客户端

### 10.1 客户端形态

- **桌面应用（主形态）**：Tauri v2 + React + HeroUI，覆盖所有核心操作。macOS / Windows / Linux。
- **CLI**：`teamai` 命令行工具，headless 操作，CI/CD 友好。
- **后台同步**：桌面端内置 SHA 轮询 + 自适应退避，检测远程变更并增量更新。

### 10.2 本地目录

```text
~/.team-ai-hub/
├── db.sqlite              ← SQLite 数据库（skills 注册表 + 缓存 + 订阅）
├── skills/                ← canonical skill 文件（单一数据源）
│   ├── code-reviewer/
│   │   ├── SKILL.md
│   │   ├── manifest.yaml
│   │   └── scripts/
│   └── find-skills/
│       └── SKILL.md
├── credentials/           ← OS keychain 元数据
└── logs/                  ← 日志文件（token 自动脱敏）
```

### 10.3 lockfile

记录每个资产当前安装的精确版本：

```json
{
  "workspace": "github.com/acme/team-skills",
  "assets": {
    "code-reviewer": {
      "version": "1.4.2",
      "ref": "v1.4.2",
      "sha": "a3f5e2c...",
      "installedAt": "2026-05-26T10:00:00Z",
      "targets": ["claude-code", "cursor"]
    }
  }
}
```

### 10.4 CLI 命令

```bash
teamai login github                        # OAuth 登录
teamai workspace add acme/team-skills      # 添加订阅源
teamai workspace list
teamai search code                         # 搜索可订阅资产
teamai subscribe code-reviewer             # 订阅
teamai unsubscribe code-reviewer
teamai sync                                # 立即同步
teamai status                              # 查看本地状态
teamai versions code-reviewer              # 查看版本历史
teamai rollback code-reviewer 1.4.1        # 回滚
teamai diff code-reviewer 1.4.1 1.4.2      # 版本对比
teamai enable code-reviewer --target cursor
teamai disable code-reviewer --target codex
```

## 11. Rust Native Installer 与 Runtime 安装

MVP 使用 Rust 原生 installer + SQLite 注册表，把 Skill 安装到各 agent 期望的位置。Team AI Hub 负责订阅、版本、权限提示、policy 和 rollback。

### 11.1 架构

- **Canonical 数据目录**：`~/.team-ai-hub/skills/{id}/` 存放 skill 的唯一真实副本。
- **Symlink 分发**：启用某个 IDE 时，创建 symlink 指向 canonical 目录。
- **SQLite 注册表**：`~/.team-ai-hub/db.sqlite` 记录所有 skill 的元数据、启用状态、变更 hash。
- **导入时 copy**：从 IDE 目录导入 skill 时，copy 到我们的数据目录（不依赖原始位置）。
- **设置可选 copy 模式**：Windows 或不支持 symlink 的环境可切换为 copy 模式。

### 11.2 支持的 Runtime（15+）

| Agent | Global Path |
|-------|-------------|
| Claude Code | `~/.claude/skills/` |
| Cursor | `~/.cursor/skills/` |
| Codex | `~/.codex/skills/` |
| Gemini CLI | `~/.gemini/skills/` |
| GitHub Copilot | `~/.copilot/skills/` |
| Windsurf | `~/.codeium/windsurf/skills/` |
| OpenCode | `~/.config/opencode/skills/` |
| Kiro CLI | `~/.kiro/skills/` |
| Roo Code | `~/.roo/skills/` |
| Continue | `~/.continue/skills/` |
| Hermes Agent | `~/.hermes/skills/` |
| Trae | `~/.trae/skills/` |
| Cline / Dexto / Warp | `~/.agents/skills/` |
| Goose | `~/.config/goose/skills/` |
| Devin | `~/.config/devin/skills/` |

> 参考 [vercel-labs/skills](https://github.com/vercel-labs/skills) 支持 50+ agent。

### 11.3 开关逻辑

- **启用** = 创建 symlink（或 copy）从 canonical 目录到 IDE 目录
- **禁用** = 删除 symlink（canonical 目录不动，skill 不会消失）
- **导入** = 从 IDE 目录 copy 到 canonical 目录 + 注册到 SQLite
- **取消托管** = 从 SQLite 删除 + 把 symlink 还原为真实文件副本

### 11.4 变更检测

- **mtime 预检**：收集所有文件的修改时间，和上次记录对比（纳秒级，不读文件内容）
- **hash 确认**：mtime 变化时才计算完整 SHA-256 hash 确认是否真的改了
- **发布时自动 bump version**：用户选择 patch/minor/major，自动更新 manifest

### 11.5 用户控制

每个 skill 可以独立控制在哪些 IDE 启用：

```
┌─────────────────────────────────────────────────────┐
│ SKILL          │ Claude │ Cursor │ Codex │ Gemini │
├─────────────────────────────────────────────────────┤
│ code-reviewer  │  ✓    │   ✓   │   ✓   │   ✗   │
│ find-skills    │  ✓    │   ✗   │   ✓   │   ✓   │
└─────────────────────────────────────────────────────┘
```

## 12. Web UI

### 12.1 主要页面

- **首页（Dashboard）**：我订阅了什么、有几个待更新、最近活动。
- **Workspaces**：我有权限的所有 Workspace（=GitHub repo）列表。
- **Workspace 详情**：仓库内所有 Skills 列表、README、成员。
- **Skill 详情**：概览、版本列表、版本对比、订阅按钮、安装说明、贡献者。
- **订阅管理**：当前所有订阅、更新策略、目标 runtime。
- **发布管理**：本地个人 Skill 发布到团队 Workspace 的 PR 状态、policy 结果、auto-merge 状态。
- **邀请中心**：邀请成员、查看 pending invitation、接受邀请后的 onboarding。
- **设置**：账户、Provider 连接、CLI 设备管理、Bot 安装状态。

### 12.2 Skill 详情关键能力

- 渲染 SKILL.md（markdown + frontmatter 解析）。
- 版本下拉切换。
- 任意两版本一键 diff（文件 diff + 语义 diff 两种视图）。
- "在 Claude Code 中订阅" / "在 Cursor 中订阅" 一键深链到本地客户端。
- 订阅人数、最近活跃订阅者、变更日志。

### 12.3 非工程师友好

- 不要求看懂 git。
- "订阅" 按钮代替 "git clone"。
- 自然语言搜索（"帮我审 PR 的 skill"）。
- 默认 stable 通道，不暴露 beta 切换给普通用户。

## 13. 安全与权限

### 13.1 权限完全继承自 Provider

- 用户能看到哪些 Workspace = 用户在 GitHub 上能看到哪些 repo。
- 用户能修改哪些 Skill = 用户对该 repo 的 write 权限。
- 团队管理员 = repo admin。
- 用户能邀请谁 = 用户在 Provider 上是否有 repo admin、org owner 或 team maintainer 权限。

我们不存权限规则，每次操作通过 Provider API 实时检查。

### 13.1.1 Bot 不提升用户权限

Team AI Hub Bot 的 token 只用于执行已授权操作。任何写入、PR、merge、邀请动作前，都必须先用发起人的 Provider 身份检查权限：

- 发起人没有 read 权限：不可见，不可安装。
- 发起人没有 write 权限：不可 publish 到团队 repo。
- 发起人没有 admin / owner / maintainer 权限：不可通过 Team AI Hub 发邀请或 auto-merge。
- public repo 的外部用户：可以浏览和安装公开 Skill，publish 只能走 fork PR / 外部贡献流程。

### 13.2 我们不存 Skill 内容

仓库内容存在 Provider 那边，我们只缓存元数据用于加速浏览（带 ETag，过期重新拉取）。下载资产时，由本地客户端直接通过 Provider API 拉取，或者跳转到 release tarball，**Skill 二进制内容不流经我们的服务器**。

这一条同时简化合规：用户的代码所有权 100% 在 Provider 那边，我们只是体验层。

### 13.3 高风险权限提示

Skill 的 manifest 声明 `permissions`。安装时 CLI 和 Web UI 都会高亮显示，特别是 `shell.execute`、`network.external`、`filesystem.write` 这类。用户主动确认后才安装。

### 13.4 Token 管理

- 每个 Provider 一个 OAuth token，存在系统 keychain。
- CLI 使用设备级 token，可在 Web UI 撤销。
- 不存储用户的 Provider 密码。

### 13.5 注册与登录

Team AI Hub 的注册就是 Provider 登录：

- 已有 GitHub 账号：点击 "Continue with GitHub" 即可进入。
- 受邀但没有 GitHub 账号：先跳转到 GitHub 注册，再回到 Team AI Hub 接受邀请。
- 企业内网：使用自托管 Provider 的 OAuth / SSO / PAT，Team AI Hub 不直接管理企业身份源。

我们可以优化注册引导和邀请落地页，但不直接创建或托管 GitHub / GitLab / Gitea 账号。

## 14. 部署模式

### 14.1 SaaS（默认）

- Web UI + 同步协调服务由我们托管。
- 用户用 GitHub OAuth 登录。
- 我们只存：用户基本信息、订阅声明、缓存的 manifest 元数据、统计数据。
- 资产内容流不经过我们。

### 14.2 自托管

提供 Docker Compose 一键部署：

- 一个 API 服务（Node.js / Go）。
- 一个 Postgres 用于元数据。
- 一个 Web UI（静态资源）。

适合：企业内网、对数据敏感的团队、使用自建 GitLab / Gitea 的场景。

### 14.3 完全本地（CLI Only）

不连接任何 Web 服务，CLI 直接对接 Git Provider：

```bash
teamai --no-cloud subscribe github.com/acme/team-skills/code-reviewer
```

这是最小可用形态，确保用户在没有云服务的情况下也能用核心功能。

## 15. MVP 范围

按 vibecoding 节奏，目标是快速跑通端到端闭环。

### 15.1 包含（已实现）

- GitHub Device Flow 登录。
- 添加 GitHub repo 作为 Workspace。
- 仓库内 Skill 自动识别（基于 `SKILL.md` 或 `manifest.yaml`）。
- 桌面端浏览 Workspace + Skill 列表 + 文件树 + 内容查看/编辑。
- CodeMirror 多语言编辑器（10+ 语言语法高亮）。
- 版本历史（commit timeline）+ 分支切换。
- Rust native installer 安装到 15+ IDE（symlink 架构）。
- SQLite 本地数据管理（skills 注册表 + 缓存 + 订阅）。
- 从 IDE 导入未托管 skill + 取消托管。
- 本地变更检测（mtime 预检 + SHA-256 hash）。
- 远程变更检测（SHA 轮询 + 增量 diff + 自适应退避）。
- 个人 Skill publish 到团队 repo：Bot 创建 PR。
- 邀请团队成员 + 修改权限。
- GitHub Discussions 集成（点赞 + 评论）。
- 高风险权限提示。
- 暗色/亮色主题 + 4 种重点色。
- 缓存管理（按 workspace 查看大小 + 清理）。

### 15.2 不在 MVP（后续阶段）

- 版本对比（语义 diff 视图）。
- Webhook 实时推送（当前用轮询替代）。
- 管理员 Dashboard（统计面板）。
- 其他资产类型（Prompt / Workflow / Knowledge）。
- GitLab / Gitea Provider。
- 团队级订阅文件。
- 计费。
- 自托管部署的 Helm chart / Terraform。
- 公开市场 / 评分 / 推荐。
- 企业 IdP / SCIM 成员同步管理。

## 16. 路线图

### Phase 1 — MVP 端到端闭环
GitHub Provider + Web UI + CLI + `skills` CLI 封装 + publish PR + 邀请中心 + 订阅自动更新。

### Phase 2 — 团队策略与设备管理
团队级 `.teamaihub/subscriptions.yaml`、设备管理、publish policy、auto-merge policy、更多 agent target 配置。

### Phase 3 — 多资产类型
Prompt、Workflow JSON、Knowledge Pack。复用同一套订阅 + 同步机制，新增解析器与发布/安装策略。

### Phase 4 — 多 Provider
GitLab、Gitea、Bitbucket、自建 Git。

### Phase 5 — 企业版
SAML SSO、自托管部署、审计日志增强、SLA 支持、合规认证。

### Phase 6 — 社区与市场
公开 Workspace 发现、订阅排行、贡献者画像、（可选）付费订阅。

## 17. 关键指标

### 17.1 用户增长
- 注册用户数。
- 连接的 Workspace 数量。
- 周活、月活。

### 17.2 资产健康
- 平均每 Workspace 的 Skill 数量。
- Skill 平均版本迭代频率。
- 团队成员订阅覆盖率。

### 17.3 同步可靠性
- 同步成功率。
- 自动更新成功率。
- 平均更新延迟（commit → 安装到本地）。

### 17.4 跨 Agent 安装健康
- 每个 agent target 的活跃订阅数。
- Rust native installer 安装 / 更新失败率。
- 与 `skills` CLI 兼容测试失败数。

## 18. 竞争与差异化

### 18.1 我们替代谁
- 团队内部那个塞满 prompts / skills 的 Git repo + 手写脚本。
- 散落在 Notion / Confluence 里的"AI 使用规范"页面。
- 复制粘贴在 Slack 频道里的 Skill 内容。

### 18.2 我们不和谁竞争
- **GitHub / GitLab**：我们建在他们之上。
- **Anthropic / OpenAI / Cursor**：我们适配他们的 Skill 协议，扩展生态。
- **Notion / Confluence**：他们做人读文档，我们做 agent 消费的资产。

### 18.3 核心差异
1. **建在 Git Provider 之上**：复用成熟的权限、审核、版本能力，不重新发明。
2. **封装 Skills 生态**：优先复用 `skills` CLI 的跨 agent 安装能力，不把工程浪费在重复 installer 上。
3. **声明式订阅 + 自动更新**：填补 Git 没有的 pull 模型与策略化更新。
4. **版本对比与回溯一等公民**：任意两版本可视化 diff，一键回滚。
5. **个人到团队的 PR 发布流**：本地 Skill 可以标准化同步到团队 repo，来源、hash、风险一目了然。
6. **非工程师可用**：Web UI 让 PM、设计、运营也能订阅 Skills、接受邀请、进入团队 Workspace。

### 18.4 一句话差异化
> 现有方案要么被绑在单一 IDE / Agent 上，要么停在"一个共享 Git repo"的原始阶段。Team AI Hub 把它们之间的鸿沟填上 —— 借助 Git Provider 已有的协作能力，提供跨 runtime 的订阅、同步、版本管理与可视化体验。

## 19. 风险与对冲

### 19.1 GitHub 自己下场做
不太可能短期发生 —— AI 资产对 GitHub 是边缘需求。对冲：第一天就把 Provider 抽象做出来，GitHub 只是第一个适配器。

### 19.2 工程师觉得"git clone 就够了"
真实存在。对冲：把跨 runtime 自动同步、声明式订阅策略、可视化版本对比、非工程师入口这四件事做到极致 —— 这些手写 git 脚本做不到。工程师团队作为入口，混合团队作为转化点。

### 19.3 单一 Runtime 厂商把团队治理也做了
例如 Anthropic 推出官方 Skill 注册中心。对冲：跨 runtime 是天然护城河，单一厂商不会主动适配竞争对手；自托管 + 开源给企业侧增加不可替代性。

## 20. 开放问题

已初步收敛：

- Skill manifest schema：MVP 采用"必填最小集 + 可选扩展字段"，详见 `SKILL_MANIFEST_SCHEMA.md`。
- Runtime 安装：Rust native installer，不内置 Node / Bun；`skills` CLI 只作为参考实现和 debug fallback。
- 邀请：Team AI Hub 提供邀请体验，但最终成员关系必须落到 Git Provider。
- publish：个人 Skill 同步团队 Workspace 走 Bot 创建 PR，不直接写主分支。

仍需在实施过程中验证：

- Webhook 推送 vs 定时拉取，默认间隔多少？
- Rust installer 与 `skills` CLI 在 Claude Code / Cursor / Codex 上的行为差异。
- GitHub App vs OAuth App：登录、repo 访问、PR 创建、成员邀请、webhook 权限如何拆分最合理？
- 私有 Workspace 的订阅，团队成员离开时如何回收本地已安装 Skill？
- 资产内容是否完全不缓存（保护隐私）vs 缓存以加速浏览（用户体验更好）？
- 同一 Skill 不同 runtime 行为差异，如何在 UI 中表达？
- 团队订阅文件 commit 到仓库时，如何避免成员之间的偏好冲突？
- 企业 IdP / SSO / team sync 场景下，邀请能力如何降级到只读引导？

## 21. 一句话总结

> Team AI Hub = **Git Provider 上的团队 Skills 工作流层**：仓库即团队空间，PR 即发布，Provider 即权限源 —— 我们补上订阅、同步、邀请、风险提示、版本治理与非工程师入口。
