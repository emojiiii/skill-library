# Skill Library — 面向普通用户的完整 UI 重构

## 目标
把当前"开发者形态"的界面重构为**双层架构**,让纯非技术用户开箱即用,同时保留创作者/管理员的完整能力。

- **消费者层(默认 · 匿名)**:发现技能 → 一键安装到 AI 工具 → 我的技能(自动更新/一键还原)
- **创作者层(GitHub 登录后解锁)**:发布/贡献、成员权限、动态 PR、CLI、源码/分支/历史、评论点赞

核心原则:版本控制与权限对普通人**隐形**(自动更新+一键还原,不暴露 commit/branch);登录从"进门的墙"变成"碰到社交/贡献功能时的即时软提示"。

## 已验证的关键事实(决定可行性)
1. **后端天然支持匿名读取**:`skill-library-provider-github` 的 token 是 `Option<String>`,仅在非空时加 `Authorization` 头;`scan_github_workspace / get_workspace_detail / get_skill_detail / read_skill_file` 全部 `token.or_else(saved_github_token)`,无 token 时降级匿名。→ **消费层不需要改 Rust 读取逻辑**。
2. **强制 token 的命令恰好都是创作者动作**:`list_github_workspaces`(列我的仓库)、`invite`、`publish` → 自然归入创作者层。
3. **skills.sh 提供可用注册表 API**:`GET https://skills.sh/api/search?q=<keyword>` 返回 JSON,每条含 `source`(GitHub `owner/repo`)、`skillId`、`name`、`installs`。这个 `source` 正好喂给现有匿名读取路径。
4. **该 API 无 CORS 头** → webview 直接 fetch 会被拦截,**必须新增一个 Rust 代理命令**(服务端请求,无 CORS 问题,与现有架构一致)。

## 现有结构基线(参考)
- 栈:Tauri v2 + React 19 + TanStack Router/Query + Zustand + HeroUI + Tailwind v4
- 登录硬墙:`RootLayout.tsx` 中 `if (!isAuthenticated) return <LoginScreen/>`
- 7 页 / 3 组导航;技能详情 5 Tab(源码/元数据/历史/评论/风险)
- i18n:`useLocale.ts` 单文件 zh/en 字典

---

## 实施分阶段(按"一次性完整重构"目标,但拆成可验证的提交)

### 阶段 1 — 拆登录墙 + 应用模式
**目标**:整个 app 匿名可用,登录改为即时触发。
- `RootLayout.tsx`:删除硬登录墙,始终渲染主壳层;`LoginScreen` 改为按需弹出(由"即时登录意图"触发)。
- `state/appStore.ts`:新增
  - `pendingAuthIntent: null | { action: 'comment' | 'publish' | 'invite' | 'browsePrivate'; resume?: () => void }` —— 即时登录后继续原动作
  - 派生 `isCreatorMode = Boolean(authLogin)`(登录即创作者层可见)
- 即时登录组件:登录成功后执行 `resume()` 回到用户刚才的动作。

**验证**:未登录能进主界面;点"评论/发布"弹登录;登录后自动继续。

### 阶段 2 — skills.sh 注册表接入(Rust 代理 + TS 层)
- **Rust**(`apps/desktop/src-tauri/src/lib.rs`):新增命令
  - `search_skills_registry(query: String) -> Vec<RegistrySkill>`:`reqwest` GET `https://skills.sh/api/search?q=`,解析 `{skills:[{id, skillId, name, installs, source}]}`,做 5–15 分钟内存缓存防限流。
  - 在 `tauri::generate_handler!` 注册。
- **TS**(`src/lib/registry.ts`,新文件):`RegistrySkill` 类型 + `searchSkillsRegistry(query)` 调上面命令;非 Tauri 环境返回 `[]`。
- 内置兜底:配置里放一小份"精选技能"列表(`owner/repo`),离线或 API 失败时展示,保证首屏永不空白。

**验证**:输入关键词能返回 skills.sh 结果;断网回落到精选列表。

### 阶段 3 — 消费者首屏:发现页 + 一键安装
- `pages/DiscoverPage.tsx`(新):搜索框 + 应用商店式卡片网格(名称、一句话描述、安装量、`isOfficial` 徽章、安全标签、[安装到我的 AI 工具])。
- `routes/DiscoverRoute.tsx`(新):`searchSkillsRegistry` 拿列表 → 选中后用**匿名** `getSkillDetail({workspace: source, ...})` 读详情(复用现有命令)。
- `widgets/ConsumerSkillCard.tsx`(新):卡片。
- `widgets/InstallToToolsDialog.tsx`(新):选 Claude Code / Cursor / Codex(默认全选)→ 调现有 `installSkill` / `subscribeWorkspaceSkill`;这是"跨端同步"主线的落点。
- 高风险技能:安装前弹**大白话**确认("这个技能会运行脚本,可能修改你的文件,确定吗?"),复用 `riskRequiresConfirmation`。

**验证**:从发现页选技能 → 一键装到多个工具 → "我的技能"出现。

### 阶段 4 — 安全卡(普通人唯一必须看到的东西)
- `widgets/SkillSafetyCard.tsx`(新):把开发者 `SkillRiskPanel` 的 risk level / permissions 翻译成大白话徽章:
  - ✅ 安全 · 只读取文件,不会修改系统
  - ⚠️ 会修改文件 / 运行脚本 / 访问网络(按 `effectiveRisk` + `permissions` 映射)
- `utils/risk.ts`:新增 `consumerRiskLabel(risk)`、`consumerSafetySummary(manifest)` 等大白话辅助函数(不动现有开发者逻辑)。
- 消费者技能详情 = 安全卡 + 描述 + 安装按钮(**不含源码 Tab、文件树、分支选择器**)。

**验证**:同一技能在消费视图只见安全卡与安装,在创作视图仍见 5 Tab。

### 阶段 5 — 我的技能(版本控制隐形化)
- 复用 `LocalPage` 数据(`dbListSkills` / 运行时开关),重做为消费者友好视图:
  - 每个技能:跨端开关(Claude/Cursor/Codex)、**自动更新**开关、**一键还原**按钮
  - 隐藏 `linkMode`、`sourceBranch@`、`isModified` 等 Git 术语(或折叠进"详情")
- 还原 = 复用现有 rollback/restore 能力,文案改为"恢复到上一个正常版本"。

**验证**:开关跨端生效;自动更新可切;一键还原可用。

### 阶段 6 — 导航重构 + 创作者层收纳
- `utils/navigation.ts`:新增 `discover`、`my-skills` 页;重组导航分组:
  - 默认(始终可见):**发现**、**我的技能**
  - 创作者(仅登录后显示):技能库(workspaces)、发布、成员、动态、CLI
- `shell/Sidebar.tsx`:创作者组在 `isCreatorMode` 为真时才渲染;底部账号区:未登录显示"登录以发布/评论"。
- `router.tsx`:新增 `/discover`、`/my-skills` 路由;**首屏默认进 `/discover`**(替换现有"自动跳第一个工作区"),仅当有已保存工作区且登录时才保留旧行为。

**验证**:匿名只见两个入口;登录后创作者入口出现。

### 阶段 7 — 术语本地化与文案
- `hooks/useLocale.ts`:新增消费者 copy keys,并把外露术语改写:
  - 工作区 → 技能库 / 来源仓库;发布 PR → 贡献修改;订阅 → 关注;CLI 区不在消费层出现
  - 新增:`discover.*`、`mySkills.*`、`safety.*`、`install.toTools.*`、`auth.justInTime.*`
- 评论/点赞入口保留,但未登录时点击 → 阶段 1 的即时登录提示。

**验证**:消费层无 Git 术语;中英文齐全(字典 zh/en 同步)。

---

## 涉及文件清单

**新增**
- `src-tauri/src/lib.rs` 内新增命令(改现有文件)
- `src/lib/registry.ts`
- `src/pages/DiscoverPage.tsx`
- `src/routes/DiscoverRoute.tsx`
- `src/pages/MySkillsPage.tsx`(或在 LocalPage 基础上拆消费视图)
- `src/widgets/ConsumerSkillCard.tsx`
- `src/widgets/SkillSafetyCard.tsx`
- `src/widgets/InstallToToolsDialog.tsx`
- `src/shell/JustInTimeAuth.tsx`

**修改**
- `src/shell/RootLayout.tsx`(拆墙 + 即时登录)
- `src/shell/Sidebar.tsx`(双层导航)
- `src/utils/navigation.ts`(新页+分组)
- `src/router.tsx`(新路由+默认首屏)
- `src/widgets/SkillDetail.tsx`(消费/创作分流,或抽 ConsumerSkillDetail)
- `src/utils/risk.ts`(大白话辅助)
- `src/hooks/useLocale.ts`(术语+新文案)
- `src/state/appStore.ts`(模式+登录意图)

## 一个待你确认的取舍(写进计划默认值)
- **数据来源**:默认以 **skills.sh `/api/search` 为主**(覆盖广、有安装量),**内置精选列表为离线兜底**。若你更想纯内置不依赖外部站,阶段 2 可只保留精选列表。计划默认采用"skills.sh 为主 + 内置兜底"。

## 验证策略
- 每阶段后:`pnpm --dir apps/desktop check`(tsc) 通过;关键路径手动冒烟。
- Rust 命令:`cargo check -p` 对应 crate;代理命令加超时与错误降级。
- 全程不破坏创作者层现有功能(回归:登录后发布/成员/活动可用)。
