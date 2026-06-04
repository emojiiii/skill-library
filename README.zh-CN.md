# Skill Library

[English](./README.md)

**Skill Library 是一个本地优先的桌面应用与 CLI，用来发现、审查、安装、同步和发布团队 AI Skills。**

## 产品定位

Skill Library 是构建在 Git Provider 之上的 AI Skills 工作流层。团队可以继续把 Skills 放在 GitHub、GitLab、Gitee、WebDAV 或自建代码平台里，Skill Library 负责把这些仓库变成可浏览、可订阅、可审查、可安装、可回滚的团队资产库。

它不重新发明账号、权限、PR、审计和版本历史，而是复用 Git 平台已有能力，再补上 AI Skills 场景里真正缺的一层：跨 Agent 安装、订阅同步、风险提示、版本治理、协作发布和本地可用性。

## 它解决什么问题

- **团队 Skills 难分发**：每个人手动复制文件，版本不一致，更新不可控。
- **非工程师难使用**：仓库目录、分支、tag、manifest 对非工程角色不友好。
- **跨工具重复维护**：同一个 Skill 可能要分别放到 Claude Code、Cursor、Codex 等不同目录。
- **安装前缺少信任判断**：脚本、权限、依赖和文件变更需要在安装前被看见。
- **个人成果难进入团队流程**：本地写好的 Skill 需要通过可审查的 PR 进入团队仓库。

## 核心功能

- **Workspace 管理**：把一个 Git 仓库作为一个团队或个人 Skill 空间。
- **Skills 浏览与搜索**：扫描 `SKILL.md`、frontmatter 和兼容 manifest，展示可安装资产。
- **多 Provider 架构**：已有 GitHub 能力，并包含 GitLab、Gitee、WebDAV provider crate 与抽象层。
- **跨 Agent 安装**：将一份 canonical Skill 安装或链接到 Claude Code、Cursor、Codex 等 runtime。
- **订阅与同步**：订阅远程 Skills，按策略拉取更新，并写入本地 lock 状态。
- **版本对比与回滚**：比较两个 ref/tag 的文件差异和语义变化，并可回滚到旧版本。
- **风险确认**：对中高风险安装、更新、回滚和发布动作要求明确确认。
- **AI Review**：对本地或远程 Skill 做安全和质量审查，帮助判断是否值得安装。
- **发布 PR 工作流**：把本地 Skill 打包、生成风险摘要，并向团队 workspace 创建发布 PR。
- **协作治理**：查看 PR、评论、合并或关闭发布请求；管理邀请、活动和通知。
- **诊断导出**：导出脱敏日志与本地状态，便于排查同步或安装问题。
- **本地优先**：已安装内容保存在 `~/.skill-library`，离线也能继续使用。

## 适合谁

- **个人开发者**：在多台设备和多个 Agent 之间复用自己的 Skills。
- **团队成员**：从团队仓库中浏览、订阅和安装已审核的 Skills。
- **Skill 作者**：把本地改进发布到团队空间，并通过 PR 接受审查。
- **团队管理员**：复用 Git 平台权限、成员、审计和分支保护来治理 AI 资产。

## 快速开始

```bash
pnpm install
pnpm dev
```

运行桌面 Web 预览：

```bash
pnpm dev:web
```

运行 CLI：

```bash
cargo run -p skill-library-cli -- --help
```

常用检查：

```bash
pnpm -r check
cargo check --workspace
cargo test --workspace
```

## 项目结构

```text
apps/desktop/                 Tauri v2 + React desktop app
  src/                        Frontend source
  src-tauri/                  Rust command layer and desktop backend
crates/skill-library-cli/     Rust CLI
crates/skill-library-core/    Shared models, paths, config, credentials
crates/skill-library-installer/
                              Runtime install/remove/link logic
crates/skill-library-manifest/
                              Skill parsing, metadata, risk and semantic changes
crates/skill-library-provider/
                              Provider traits and shared provider models
crates/skill-library-provider-github/
                              GitHub implementation
crates/skill-library-provider-gitlab/
                              GitLab implementation
crates/skill-library-provider-gitee/
                              Gitee implementation
crates/skill-library-provider-webdav/
                              WebDAV implementation
crates/skill-library-publish/ Publish package and policy logic
crates/skill-library-sync/    Subscription, sync, diff and rollback logic
docs/                         Product, architecture, schema and demo notes
scripts/                      Demo, smoke and maintenance scripts
```

## 本地数据

Skill Library 默认把托管数据放在：

```text
~/.skill-library/
  db.sqlite
  skills/
  logs/
```

canonical Skill 保存在 `~/.skill-library/skills/`，各 Agent runtime 可以通过 symlink 或 copy mode 指向这份内容。

## 许可证

MIT
