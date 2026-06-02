# Skill Library MVP 工程任务清单

> 节奏：vibecoding，速度优先。每个里程碑 1-2 周可演示，能跑通 demo 就发车，细节回头补。

## 1. MVP 目标

端到端跑通：用户在 Tauri 桌面端用 GitHub 登录 → 把一个 GitHub repo 挂成 Workspace → React + HeroUI UI 浏览仓库内的 Skill 列表/详情/版本 → 订阅 Skill → Rust native installer 安装到 Claude Code / Cursor / Codex → 本地个人 Skill publish 到团队 Workspace 并创建 PR → 管理员在 Skill Library 邀请成员 → 仓库有新 commit/tag 时本地自动更新 → 任意两版本可视化 diff → 一键回滚到旧版本。一条主流程闭环，GitHub 一个 Provider，本地安装不内置 Node/Bun。

## 2. 技术栈

合理默认，开工后可调整：

- **桌面端**：Tauri v2 + Rust core + React 19 + Vite
- **UI**：HeroUI v3 + Tailwind CSS v4 + lucide-react + TanStack Router
- **云端控制面**：Node.js + TypeScript + Hono + Postgres（只做 Bot、webhook、订阅同步、邀请状态）
- **CLI**：Rust，复用 `crates/skill-library-*` core，不单独做 Node CLI
- **Provider SDK**：Rust `reqwest` 直调 GitHub REST/GraphQL；云端可用 Octokit
- **认证**：GitHub OAuth / Device Flow + 后端 session（cookie）+ CLI device token；Bot / PR / invitation 能力优先评估 GitHub App
- **Skills 安装**：Rust native installer；`skills` CLI 仅作为参考实现和 debug fallback，不内置 Node/Bun
- **部署**：Docker Compose 自托管 / Vercel + Neon/Supabase SaaS 双轨
- **Monorepo**：Cargo workspace + pnpm workspace，包结构详见 `TAURI_TECHNICAL_PLAN.md`
- **测试**：Rust 单测 + vitest 少量前端单测 + Playwright/Tauri E2E
- **日志**：Rust `tracing` + 云端 pino

## 3. 里程碑划分

### M0 项目脚手架（1 周）

| 编号 | 标题 | 验收标准 | 估时 | 依赖 |
|---|---|---|---|---|
| M0-1 | 初始化 Cargo + pnpm workspace | Cargo workspace + `apps/desktop` pnpm workspace 跑通，根目录有 fmt/clippy/test/lint 基线 | 半天 | - |
| M0-2 | 搭 Tauri v2 + React + HeroUI | `cargo tauri dev` 打开桌面窗口，React 19 + HeroUI v3 + Tailwind v4 可用 | 1 天 | M0-1 |
| M0-3 | 搭 Rust core crate | `skill-library-core`、`skill-library-manifest`、`skill-library-installer` 空 crate 跑通单测 | 半天 | M0-1 |
| M0-4 | 搭 Rust CLI 骨架 | `skill-library --version` 可执行，复用 core crate，不依赖 Node runtime | 半天 | M0-3 |
| M0-5 | 搭云端 API 骨架 | Hono `GET /health` 返回 200，Postgres docker-compose 起得来 | 1 天 | M0-1 |
| M0-6 | 本地 SQLite + keychain spike | Tauri command 能写 SQLite，Rust core 能写/读 OS keychain | 1 天 | M0-2,M0-3 |
| M0-7 | deep link + single instance | `skill-library://subscribe?...` 能唤起已有桌面窗口并传给前端 | 1 天 | M0-2 |

### M1 GitHub 登录 + Provider + Bot Spike（1.5 周）

| 编号 | 标题 | 验收标准 | 估时 | 依赖 |
|---|---|---|---|---|
| M1-1 | 注册 GitHub OAuth App / GitHub App spike | 跑通用户登录；同时验证 GitHub App 对 PR、webhook、成员邀请的权限边界，写入 .env.example | 1 天 | M0-2 |
| M1-2 | Tauri GitHub 登录流程 | 桌面端打开 GitHub 授权 → deep link / loopback 回调 → token 写入 keychain → 前端拿到当前用户 | 1 天 | M1-1,M0-7 |
| M1-3 | 持久化用户与 token | `users` + `provider_tokens` 表，token 加密存储（简单 AES） | 半天 | M1-2,M0-5 |
| M1-4 | 定义 Rust `Provider` trait | trait 含 list_repos / get_tree / get_file / list_tags / compare_refs / list_members / register_webhook / create_invitation / create_pull_request | 半天 | M0-3 |
| M1-5 | GitHub Provider 实现（读取） | 实现 list_repos / get_tree / get_file / list_tags / compare_refs，REST + GraphQL 混用，单测覆盖正反例 | 2 天 | M1-3,M1-4 |
| M1-6 | 桌面端列出我的 Workspaces | Tauri command 返回当前用户在 GitHub 上有权限的 repo 列表（带分页与缓存） | 1 天 | M1-5 |
| M1-7 | CLI：`skill-library login github` | 走 device flow（或 PAT 兜底），token 写到 OS keychain | 1 天 | M1-1,M0-4 |

### M2 Skill 识别 + Web UI 浏览（2 周）

| 编号 | 标题 | 验收标准 | 估时 | 依赖 |
|---|---|---|---|---|
| M2-1 | Manifest 解析器 | 输入仓库目录树 → 输出 Skill 列表，支持 `manifest.yaml` 与 `SKILL.md` frontmatter 两种来源，Rust 校验 schema | 1 天 | M0-3 |
| M2-2 | Workspace 详情 command | Tauri command 返回 Skill 列表 + README，带本地 SQLite 缓存 | 1 天 | M1-5,M2-1 |
| M2-3 | Skill 详情 command | Tauri command 返回 manifest + SKILL.md 渲染源 + 版本列表（git tags） | 1 天 | M2-2 |
| M2-4 | Desktop：Workspaces 列表页 | 表格展示用户的 repo，可"添加为 Workspace"，搜索框过滤 | 1 天 | M1-6 |
| M2-5 | Desktop：Workspace 详情页 | 列出仓库内 Skill 列表 + README 渲染 | 1 天 | M2-2 |
| M2-6 | Desktop：Skill 详情页 | 渲染 SKILL.md（markdown），版本下拉切换查看历史版本，订阅按钮（占位，M3 接通） | 2 天 | M2-3 |
| M2-7 | "添加 Workspace" 流程 | 点击后写入 `workspaces` 表，记录 owner/repo/默认分支 | 半天 | M2-2 |

### M3 CLI 订阅 + 同步 + Rust installer（2 周）

| 编号 | 标题 | 验收标准 | 估时 | 依赖 |
|---|---|---|---|---|
| M3-1 | 本地配置目录初始化 | `~/.skill-library/` 目录结构按文档 10.2 创建，缺失自动建 | 半天 | M1-7 |
| M3-2 | `subscriptions.yaml` 读写 | 解析、校验、原子写入；CLI `skill-library subscribe/unsubscribe` 修改它 | 1 天 | M3-1,M0-6 |
| M3-3 | 资产下载器 | 给定 owner/repo/ref，通过 GitHub release tarball 或 `git archive` 下载到 `cache/`，校验 sha | 1 天 | M1-5 |
| M3-4 | Rust native installer | 实现 install/list/remove/update，支持 copy、原子 swap、path traversal 防护、lockfile 写入 | 2 天 | M0-6 |
| M3-5 | 三 runtime 安装 smoke test | 用 Rust installer 把同一 Skill 安装到 Claude Code / Cursor / Codex，记录路径和失败模式，并和 `skills` CLI 做行为对比 | 1 天 | M3-3,M3-4 |
| M3-6 | `skill-library sync` 主流程 | 读订阅 → 拉 manifest → 决策 → 下载 → 装 → 写 lockfile，串行实现先跑通 | 2 天 | M3-2,M3-3,M3-5 |
| M3-7 | `skill-library status` / `versions` | 展示当前订阅状态、本地装的版本、远端最新版本 | 半天 | M3-6 |
| M3-8 | Web 订阅按钮深链 CLI | Web 上点"订阅" → `skill-library://subscribe?...` deeplink → CLI 接住并写订阅 | 1 天 | M2-6,M3-2 |

### M4 Publish PR + Bot 工作流（1.5 周）

| 编号 | 标题 | 验收标准 | 估时 | 依赖 |
|---|---|---|---|---|
| M4-1 | Publish 权限校验 | `skill-library publish` 前用 Provider API 确认发起人对目标 repo 有 write 权限；无权限时拒绝 | 半天 | M1-5,M3-1 |
| M4-2 | 本地 Skill 打包与 provenance | 读取本地 Skill，生成 sha256、manifest summary、risk summary、source metadata | 1 天 | M2-1 |
| M4-3 | Bot 创建 branch + PR | Bot 把 Skill 提交到目标 repo 的 branch，创建 PR，PR body 含来源人、来源路径、hash、风险等级 | 2 天 | M1-1,M4-1,M4-2 |
| M4-4 | Policy check | CI/API 校验 schema、危险权限、scripts、大文件；结果写回 PR check/status | 1 天 | M4-3 |
| M4-5 | Auto-merge 策略 | low risk + trusted user + schema pass 的 PR 可自动合并；其他 PR 等待人工 review | 1 天 | M4-4 |
| M4-6 | Web 发布管理页 | 展示 publish PR 状态、policy 结果、是否 auto-merged | 1 天 | M4-3,M4-4 |

### M5 版本对比 + 回滚（1 周）

| 编号 | 标题 | 验收标准 | 估时 | 依赖 |
|---|---|---|---|---|
| M5-1 | API：两版本文件 diff | `GET /api/.../skills/:id/diff?from=&to=` 返回 unified diff，按文件分组 | 1 天 | M2-3 |
| M5-2 | API：语义 diff | 解析两版 manifest，返回 added/removed/changed 的 fields 数组（permissions、targets、version 高亮） | 1 天 | M2-1,M5-1 |
| M5-3 | Web：版本对比页 | 选两个 tag → tab 切换 文件 diff / 语义 diff，关键字段标红 | 1.5 天 | M5-1,M5-2 |
| M5-4 | CLI `skill-library diff` | 输出彩色 unified diff + 关键字段汇总 | 半天 | M5-1 |
| M5-5 | CLI `skill-library rollback` | 修改 lockfile 指向旧 tag → 重装该 Skill；不动远端仓库 | 1 天 | M3-6 |
| M5-6 | 回滚 E2E 验证 | 装 v1.4.2 → 回滚到 v1.4.1 → Claude Code / Cursor / Codex 三边都生效 | 半天 | M5-5,M3-6 |

### M6 邀请中心 + 自动更新（2 周）

| 编号 | 标题 | 验收标准 | 估时 | 依赖 |
|---|---|---|---|---|
| M6-1 | API：注册 GitHub webhook | 添加 Workspace 时自动创建 push webhook，签名校验 | 1 天 | M1-5 |
| M6-2 | Webhook 接收端 | `POST /api/webhooks/github` 验签 → 更新缓存的 manifest → 发"有更新"事件 | 1 天 | M6-1 |
| M6-3 | 客户端通知通道 | 简单方案：CLI 启动时拉 `GET /api/notifications`；进阶：SSE。MVP 选拉模式 | 1 天 | M6-2 |
| M6-4 | 定时同步守护 | `skill-library daemon`（或 `sync --watch`）按配置间隔（默认 1h）轮询 + 拉通知 | 1 天 | M3-6,M6-3 |
| M6-5 | 更新策略执行 | auto-patch / auto-minor / manual / pin 四种策略在 sync 决策时正确生效 | 1 天 | M3-6 |
| M6-6 | 邀请权限校验 | repo collaborator / org member + team invitation 前，确认发起人有 admin / owner / maintainer 权限 | 1 天 | M1-5 |
| M6-7 | 邀请 API + Web UI | Web 输入 GitHub username/email → Provider 发 invitation → pending invitation 状态可见 | 1 天 | M6-6 |
| M6-8 | 受邀用户 onboarding | 无 GitHub 账号时引导注册；登录后回到 invitation landing，接受后进入 Workspace | 1 天 | M6-7 |
| M6-9 | 失败回滚 | 安装失败时自动恢复上一个 working 版本 + lockfile，CLI 给出清晰错误 | 1 天 | M3-5,M5-5 |
| M6-10 | 高风险权限提示 | 安装/更新/publish 前若 manifest 含 `shell.execute` / `network.external` / `filesystem.write`，CLI + Web 双端高亮提示并要求确认 | 1 天 | M3-6,M2-6,M4-2 |

### M7 收尾（1 周）

| 编号 | 标题 | 验收标准 | 估时 | 依赖 |
|---|---|---|---|---|
| M7-1 | 错误处理统一 | API 错误响应统一 schema；CLI 错误码与友好文案；Web 全局 toast | 1 天 | 全部 |
| M7-2 | 关键路径 E2E 测试 | Playwright 跑通：登录 → 加 workspace → 看 skill → 订阅 → CLI sync → 装好 | 1 天 | 全部 |
| M7-3 | 安装文档 | README + QUICKSTART：本地 dev、CLI 安装、demo repo 搭建 | 半天 | 全部 |
| M7-4 | Demo Skill 仓库 | 准备一个 `skill-library-demo-skills` 公开 repo，含 2 个 Skill、3 个 tag | 半天 | - |
| M7-5 | Docker Compose 自托管包 | `docker compose up` 起 api + web + db，环境变量文档化 | 1 天 | 全部 |
| M7-6 | Demo 录屏 + 演讲稿 | 按本文 §6 脚本完整跑一遍，录 5 分钟视频 | 半天 | M7-2,M7-4 |

## 4. 横切任务

不挂在某一个里程碑里，每个里程碑都顺手做一点：

- **日志**：后端 pino 结构化输出；CLI 写到 `~/.skill-library/logs/{date}.log`，可 `--verbose` 输出到控制台。
- **错误处理**：定义 `SkillLibraryError` 基类（含 code、userMessage、cause），三端共享；M7-1 统一收口。
- **测试**：核心包（manifest 解析、provider、skills installer wrapper、publish policy、sync 决策）写单测，目标 60%+ 覆盖率。E2E 仅关键路径。
- **类型安全**：Rust domain types 是源头，必要时生成 TS 类型；Cloud API 继续用 zod/OpenAPI。
- **配置**：Rust config 用 serde 校验，Cloud env 用 zod 校验，缺失启动即失败。
- **文档**：每个里程碑结束时更新 README 的"已支持能力"清单 + 一段更新日志。

## 5. 风险与未决问题

引用伴生文档：`PROVIDER_SPIKE.md`、`SKILL_MANIFEST_SCHEMA.md`：

1. **Rust installer 兼容性**（PRODUCT_DOCUMENT §20）。需要验证 Claude Code / Cursor / Codex 的 skills 目录、project-level scope、copy/symlink 行为是否稳定，并和 `skills` CLI 做兼容测试。
2. **Skill manifest schema 严格度**。已在 `SKILL_MANIFEST_SCHEMA.md` 决策为"必填最小集 + 可选扩展字段"两层，M2-1 实现时按此落地。
3. **Provider 抽象的边界**。GitHub release tarball、git archive、GraphQL paging 行为差异大，第二个 Provider（GitLab/Gitea）接入时可能反向修改接口。`PROVIDER_SPIKE.md` 需要在 M1 期间产出，明确接口最终形态再封版。
4. **Webhook 在自托管/防火墙后场景不可达**。MVP 用拉模式兜底（M6-3），但用户体验上"准实时"会退化为分钟级，需在文档里说清楚。
5. **资产内容是否缓存到我们后端**（PRODUCT_DOCUMENT §20、§13.2）。MVP 选项：只缓存 manifest 元数据，资产内容走 GitHub 直链。需要在 M1-5 实现时锁死，避免 M6 改架构。

## 6. Demo 脚本（5 分钟）

> 设定：演示者本机已安装 Claude Code + Cursor + Codex，准备好 `skill-library-demo-skills` 仓库，含 `code-reviewer` 与 `pr-summarizer` 两个 Skill，至少 v1.0.0、v1.1.0、v1.2.0 三个 tag；GitHub App 已安装到 demo repo。

1. **打开 Web UI**：访问 `localhost:3000`，点 "Sign in with GitHub"。
   *预期*：跳转 GitHub 授权 → 回到 Dashboard，显示当前用户头像。

2. **添加 Workspace**：在 Workspaces 页搜 `demo-skills`，点击"Add as Workspace"。
   *预期*：列表出现 `demo-skills`，点进去看到两个 Skill 卡片 + README。

3. **浏览 Skill 详情**：点 `code-reviewer`。
   *预期*：渲染 SKILL.md，右侧版本下拉显示 v1.0.0/v1.1.0/v1.2.0，能切换查看历史版本。

4. **版本对比**：选 v1.0.0 和 v1.2.0，点 "Compare"。
   *预期*：tab 切换，文件 diff 显示 SKILL.md 改动，语义 diff 高亮 `permissions` 多了 `shell.execute.limited`。

5. **CLI 登录与订阅**：终端 `skill-library login github`（或粘 device code），再点 Web 上的"订阅到本机" → 系统提示打开终端。
   *预期*：CLI 收到 deeplink，提示订阅 `code-reviewer`，确认后 `subscriptions.yaml` 写入。

6. **同步**：`skill-library sync`。
   *预期*：日志显示下载 v1.2.0 → 高风险权限提示 `shell.execute.limited` → 用户输入 y → Rust installer 安装到 Claude Code / Cursor / Codex，写 lockfile。

7. **打开 Claude Code 验证**：在 Claude Code 里 `/skills`，看到 `code-reviewer`，跑一次小任务。
   *预期*：Skill 已加载并可用。

8. **多 agent 验证**：`skill-library status --targets`。
   *预期*：Claude Code / Cursor / Codex 三个 target 都显示 installed。

9. **个人 Skill 发布到团队**：本地准备 `~/.claude/skills/local-helper`，执行 `skill-library publish local-helper --workspace acme/team-skills`。
   *预期*：Bot 创建 PR，PR body 显示 Source-User、Source-Hash、Risk-Level；policy check 通过后 low risk 自动合并。

10. **邀请成员**：在 Web UI 的 Workspace 成员页输入同事 GitHub username/email，点击 Invite。
    *预期*：GitHub 发出 repo collaborator 或 org/team invitation；受邀用户登录 Skill Library 后进入 invitation landing。

11. **自动更新**：在 demo 仓库 push 一个 v1.2.1 tag → 等几秒（webhook 通道）或 `skill-library sync`。
    *预期*：CLI 提示 patch 自动更新 → 装好 v1.2.1，lockfile 更新；三个 agent target 都换成新版本。

12. **回滚**：`skill-library rollback code-reviewer 1.2.0`。
    *预期*：lockfile 指回 v1.2.0，三个 agent target 都回到旧版本；`skill-library status` 显示 "pinned at v1.2.0"。

> 收尾一句话："个人 Skill 通过 PR 进入团队仓库，团队成员一键订阅到任意 agent，邀请、更新、回滚都在一条 Git 工作流里 —— Skill Library。"
