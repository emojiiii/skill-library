# 方案：发现页精选/分类增强 + 合并「我的技能」与「Local」

## 你的三个决定
1. **合并成一个页面**（我的技能 + Local 二合一）
2. **页面叫「我的技能 (My Skills)」**，放侧边栏「你」分组
3. **发现页：扩充精选 + 分类标签**

---

## 关键事实（已核实）
- **skills.sh API 只有 `/api/search?q=` 一个端点**（trending/featured/categories 全 404），返回字段只有 `id/skillId/name/installs/source`——无 category/description/tags。所以精选和分类都得我们自己策划。
- **「我的技能」和「Local」是同一份数据的两种视图**：都查 `dbListSkills`(db-skills)，都做 per-runtime 开关。
  - 我的技能（`/my-skills`）：消费卡片、隐藏 Git、有自动更新+移除
  - Local（`/installed`）：统计卡+表格、显示 Git 元数据、有「从 IDE 导入」+「推送到 workspace」
- **推送到 workspace 的 PushModal 是全局的**（RootLayout 里，靠 zustand `setPushEntry/setPushOpen`），合并页复用同一套 store action 即可。
- 分类搜索词都验证过有结果（frontend/review/test/docs/security/api/database/git/refactor 各 100 条）。

---

## Part A — 发现页增强（registry.ts + DiscoverPage.tsx + DiscoverRoute.tsx）

### A1. 扩充精选（registry.ts）
`FEATURED_SKILLS` 从 3 个扩到 ~12 个，全部用实测存在的真实技能，例如：
- anthropics/skills: frontend-design, webapp-testing, canvas-design
- vercel-labs/agent-skills: web-design-guidelines, vercel-react-best-practices, vercel-composition-patterns
- vercel-labs/skills: find-skills
- obra/superpowers: test-driven-development, requesting-code-review
- mattpocock/skills: improve-codebase-architecture, grill-with-docs
- xixu-me/skills: github-actions-docs

### A2. 分类标签（registry.ts + DiscoverPage.tsx）
- 在 registry.ts 定义 `SKILL_CATEGORIES`：`[{ id, labelKey, query }]`，如
  前端=frontend、代码审查=review、测试=test、文档=docs、安全=security、API=api、数据库=database、Git=git、重构=refactor。
- DiscoverPage 顶部搜索框下方加一排分类「胶囊」(HeroUI Chip，可点)。点击某分类 = 把对应 `query` 灌进搜索（复用现有搜索链路）。「全部/精选」是默认态。
- DiscoverRoute：新增 `activeCategory` state；点分类时 setQuery(category.query) 并高亮该胶囊。无 query 且无分类 → 显示精选。

### A3.（顺带）精选区文案
标题区根据状态显示「精选推荐 / 分类:前端 / 搜索结果」。

---

## Part B — 合并「我的技能」与「Local」（核心）

### B1. 新的 MySkillsPage（重写 pages/MySkillsPage.tsx）
采用 **Local 的好看外观**（你认可的）+ 两边功能合集：

**顶部**：3 个统计卡（Stat 组件搬过来）——已安装数 / 启用的集成数 / 检测到的运行时数。

**工具条**：`从 IDE 导入`（复用 Local 的导入 Modal）+ `检查更新`（我的技能的 syncNow）+ `刷新`。

**主体**：技能列表。为兼顾「好看」和「消费者友好」，用**卡片式行**（比纯表格更适合消费层，又比现在的我的技能卡片更紧凑）：
- 每行：名称 + 描述 + per-runtime 开关（沿用）
- 元数据按需展示：版本、来源 workspace、modified 标记（来自 Local，但弱化呈现）
- 行操作：自动更新开关（我的技能）、推送到 workspace（Local 的 onPush）、移除（我的技能）

**空状态**：引导去发现页安装。

### B2. 数据与依赖
- 数据全部来自 `dbListSkills` / `dbListRuntimes` / `dbScanUnmanaged`（导入用），与现 Local 一致。
- 推送：复用 RootLayout 的全局 PushModal（`useAppStore` 的 setPushEntry/setPushPreview/setPushOpen）——MySkillsPage 直接读 store，不需要 props 透传。
- 导入：Local 的 importSkill mutation + Modal 搬进来。

### B3. 路由与导航清理
- `router.tsx`：删 `installedRoute`（/installed）；`/my-skills` 仍指向新 MySkillsPage。
- `navigation.ts`：从 `AppPage` 删 `installed`；navRoutes/personalPages/pageCopyKeys 移除 installed；routeToPage 删 `/installed` 分支。
- `Sidebar.tsx`：navGroups 里 `nav.tools` 分组原是 `["installed","cli"]` → 删 installed，只剩 `["cli"]`（或把 cli 也归并，保留 tools 组放 cli）。「你」组 `["discover","mySkills"]` 不变。
- 删除 `routes/InstalledRoute.tsx`、`pages/LocalPage.tsx`（功能已并入 MySkillsPage）。
- RootLayout 里 `localAgents` query 若仅 InstalledRoute 用则清理（核对后再删）。

### B4. 边界
- per-skill 自动更新仍存 localStorage（`my-skills:auto:${id}`）。
- Local 的 `onToggleRuntime`/`roots`(listLocalAgentRoots) 路径与 db-skills 开关是两套——合并后统一走 db-skills 的 enable/disable（与现 Local 主表一致）；listLocalAgentRoots 仅旧 InstalledRoute 的 props 用过，核对后清理。

---

## 影响文件
**Part A**：`lib/registry.ts`、`pages/DiscoverPage.tsx`、`routes/DiscoverRoute.tsx`、`hooks/useLocale.ts`（分类文案）
**Part B**：`pages/MySkillsPage.tsx`（重写）、`router.tsx`、`utils/navigation.ts`、`shell/Sidebar.tsx`、删 `routes/InstalledRoute.tsx` + `pages/LocalPage.tsx`、`hooks/useLocale.ts`（合并文案）、核对 `shell/RootLayout.tsx`

## 验证
- `pnpm check` + `pnpm test` + `pnpm build`。
- 你跑 `pnpm dev:web` 手测：发现页精选变多、分类胶囊可点切换；侧边栏只剩一个「我的技能」、外观是统计卡+列表、导入/推送/自动更新/移除都在、/installed 不再存在。

## 执行顺序（每步后 pnpm check）
1. Part A（发现页，独立、低风险，先交付）
2. Part B：先写新 MySkillsPage → 改 router/navigation/Sidebar → 删旧文件 → 清理 RootLayout
