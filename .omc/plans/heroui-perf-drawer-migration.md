# 方案：性能优化 + Drawer 化 + HeroUI v3 组件迁移

## 进度
- [x] 阶段 1 — 性能（后端复用 asset + 并行请求；前端详情 IndexedDB 缓存 + retry 调整）
- [x] 阶段 2 — Drawer 化（发现页右侧改 HeroUI Drawer；工作区按用户决定保留分栏）
- [x] 阶段 3 — 组件迁移
  - 第 1 波：Pill→Chip、SegmentedTabs→Tabs、删除死 .kbd
  - 第 2+3 波：新建 Card 包装组件（HeroUI Card + 扁平观感覆盖），迁移 11 个文件的 card 系列
    （CliPage/SubscriptionsPage/ActivityPage/PublishPage/InvitationsPage/LocalPage/MySkillsPage
    + MetricTile/ManagementTable/ResultBlock/SkillRiskPanel/ActivityTimeline/ComparisonView），
    删除 .card/.card-header/.card-title/.card-subtitle/.card-body 死 CSS
  - 保留：card-row（列表行）、login-card、empty-state、skill-skeleton、第三方库样式、设计 token



## 你的三个决定
1. **Drawer 范围**：发现页 + 工作区，两处右侧详情面板都改成 HeroUI Drawer
2. **样式迁移**：尽量全部迁移到 HeroUI v3 组件
3. **顺序**：性能与 UI 一起做（分阶段交付，每阶段可单独验证）

## 关键现状（已核实）
- HeroUI **v3.1.0**，组件齐全（drawer/card/chip/badge/tabs/disclosure/skeleton/scroll-shadow/empty-state/separator/tooltip 等）。目前只用了 Button/Input/Modal/Switch/Tooltip/Spinner。
- `main.tsx` **无需 HeroUIProvider**（Modal 已在 10+ 文件正常工作），Drawer 可直接用。
- 自写样式：`styles.css` 2023 行 / 273 个 class；自写组件 20+ 个。
- **性能瓶颈**：发现页点一个技能，后端 `read_skill_detail` 在「按 id 解析」路径下产生约 16 个**串行**网络往返：
  - 4 次 404 试探字面路径（manifest.yaml/yml/json + SKILL.md）
  - 整仓扫描 `scan_skill_assets_at`（其实此时已拿到 manifest，却被丢弃）
  - 再重新读一遍 manifest + readme + markdown + tags，全是顺序 `await`
  - 前端 `retry:1` 让失败时延迟翻倍，且详情**无 IndexedDB 缓存**。

---

## 阶段 1 — 性能（最高价值，先做）

### 1a. 后端：消除冗余往返（`crates/teamai-provider-github/src/scan.rs`）
- `read_skill_detail` 走「按 id 解析」分支时，`resolve_skill_dir` 已经扫描并解析出 `SkillAsset`（含 manifest）。**直接复用**这个 asset，不再二次 `read_skill_asset`。
- 解析成功后，把 readme / skill_markdown / versions 三个独立请求用 `futures::join!` **并行化**（依赖已存在 `futures`）。
- 字面路径优先逻辑保留（工作区浏览场景仍是 1 次命中）。
- 预期：发现页点击从 ~16 串行往返降到 ~3-4（get_workspace → 一次扫描/批量 → 并行 readme/md/tags）。

### 1b. 前端：缓存 + 去抖（`DiscoverRoute.tsx` / `WorkspaceSkillsRoute.tsx`）
- 详情查询接入已有的 IndexedDB 缓存层（参考 `SkillFileTree` 的 `getFileTreeFromCache`/`putFileTreeInCache` 模式），命中即秒开。
- 发现页详情 `retry` 从 1 调成 0（404 不该重试），失败立即反馈。
- `staleTime` 提到 5-10 分钟（已部分如此）。

**验证**：`cargo test -p teamai-provider-github`、`pnpm check`，并手动点发现页技能确认变快。

---

## 阶段 2 — Drawer 化右侧详情面板

### 2a. 发现页（`DiscoverRoute.tsx` / `DiscoverPage.tsx`）
- 当前右侧是常驻 `<aside>`（`lg:` 才显示）。改成 HeroUI `Drawer`（右侧 placement），点击卡片打开，含 Backdrop + CloseTrigger。
- `detailPanel` 内容基本不变，迁移到 `Drawer.Body`，安装按钮进 `Drawer.Footer`。

### 2b. 工作区（`WorkspacesPage.tsx`）
- 当前是 `react-resizable-panels` 可拖拽双栏。**取舍点**：工作区是「列表 + 详情」长期并排的工作流，改成 Drawer 会损失「边看列表边看详情」的并排体验。
- 建议：保留可拖拽分栏作为默认，**额外**提供「在 Drawer 中打开」能力；或在窄屏（面板收起）时用 Drawer。
- 若你坚持完全 Drawer 化，我会把 `SkillDetail` 包进 Drawer，但需接受并排能力下降。→ **这一点我会在实施前再跟你确认一次**。

**验证**：`pnpm check` + 手动开合 Drawer、ESC/点遮罩关闭、内容滚动。

---

## 阶段 3 — HeroUI 组件迁移（分波次，"尽量全部"）

按「收益高 / 风险低」排序，分波交付，每波后你可中途叫停：

**第 1 波（低风险、高频）**
- `Pill` → HeroUI `Chip`/`Badge`（全项目高频使用，统一观感）
- `SegmentedTabs` → HeroUI `Tabs`
- `empty-state` → `EmptyState`
- `skeleton`（WorkspacesPage 的扫描骨架）→ `Skeleton`
- `scroll-area` → `ScrollShadow`（可选）

**第 2 波（结构件）**
- `card` 系列 / `SkillSafetyCard` / `MetricTile` / `SectionHeader` → `Card` + `Typography` + `Separator`
- `RiskBadge` → `Badge`/`Chip`
- `SkillListWithFiles` 的展开 → HeroUI `Disclosure`（替换上一轮我手写的 grid 动画）
- `kbd` → `Kbd`

**第 3 波（复杂组件，谨慎）**
- `ManagementTable` / `rows.tsx` → `Table`
- `ConsumerSkillCard` / `SkillCard` → `Card`
- `DeviceCodePanel` / `SkillComments` / `ActivityTimeline` / `SkillCommitsTimeline` → 视情况用 `Card`/`Avatar`/`Separator` 重构

**保留自写（不迁移）**
- 设计 token / 主题变量（`:root`、`--xxx`、`.dark`）
- 纯布局类（`app-shell`、flex 容器）
- 第三方库定制（`mdx-editor-*`、`code-editor-*`、`markdown-*` CodeMirror/MDXEditor）

每迁移一个组件就删掉对应的死 CSS class，逐步缩小 `styles.css`。

**验证**：每波 `pnpm check`，并人工对比改造前后视觉。

---

## 交付与风控
- 三阶段**独立提交**，每阶段可单独验证回滚。
- 阶段 1 修完即可解决「慢」，建议先合。
- 阶段 3 体量大（273 class），按波次推进，不追求一次清零；保持每一步 UI 不崩。
- 2b（工作区 Drawer）实施前会再与你确认交互取舍。

## 待你确认后我立即开始的第一步
阶段 1a + 1b（性能），改动文件：
- `crates/teamai-provider-github/src/scan.rs`（复用 asset + 并行请求）
- `apps/desktop/src/routes/DiscoverRoute.tsx`、`WorkspaceSkillsRoute.tsx`（缓存 + retry）
