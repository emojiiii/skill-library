# 方案：路由结构重构 — workspace 移出 URL，改为全局选择（方向 B）

## 目标
消除「workspace 身份一会儿在 URL、一会儿在 store」的双数据源结构。让侧边栏的 workspace 选择器成为**唯一真相源**，workspace 页面变成扁平的顶级路由，从 store 读当前 workspace。从根上消灭「进个人页 → URL 丢 workspace → 选择被清空」这一整类 bug。

## 为什么是方向 B（桌面应用语境）
这是 Tauri 桌面应用：无地址栏、用户不手敲/不分享 URL。「workspace 进 URL」唯一的好处（可分享链接）在此不成立，却带来了双数据源的结构性别扭。

## 上一轮工作的衔接
上一轮加的 `appStore.selectedWorkspace` + localStorage 持久化，在方向 B 里**升级为主数据源**（不再是补丁）。`RootLayout` 里那段「URL→store 同步」的 effect 将被删除（不再需要）。

---

## 当前结构（要改的）
- `router.tsx`：`/workspace/$owner/$repo` 父路由 + 子路由（index=skills / publish / invitations / activity）。index 用 `key={owner/repo}` 在切 workspace 时强制 remount。
- `WorkspaceShell.tsx`：`useParams({ from: "/workspace/$owner/$repo" })` 取 owner/repo，提供 `WorkspaceContext`（workspace / workspaceMeta / authLogin）。
- 4 个 workspace route 组件用 `useWorkspace()` 读 context。
- `RootLayout.tsx`：`workspaceFromPathname(pathname)` 推导 workspaceRef；4 处 `navigate({ to: "/workspace/$owner/$repo", ... })`（addRemote / addManual / deepLink / sidebar onSelect）。
- `Sidebar.tsx`：`workspaceFromPathname` + `buildNavPath` 构建 workspace 导航链接。
- `navigation.ts`：`buildNavPath` / `workspaceFromPathname` / `routeToPage` / `workspaceSubPath` / `navRoutes`。

---

## 改动步骤

### 1. router.tsx — 扁平化路由
- 删除 `/workspace/$owner/$repo` 父路由及其子路由。
- 新增一个**无路径布局路由**（pathless layout route，component = `WorkspaceShell`），其子路由为顶级静态路径：
  - `/skills`   → `WorkspaceSkillsRoute`
  - `/publish`  → `WorkspacePublishRoute`
  - `/members`  → `WorkspaceInvitationsRoute`
  - `/activity` → `WorkspaceActivityRoute`
- 这样 4 个 workspace 页共享 WorkspaceShell 提供的 context，URL 里不再有 workspace 段。

### 2. WorkspaceShell.tsx — 从 store 读 workspace
- 不再用 `useParams`，改 `useAppStore((s) => s.selectedWorkspace)`。
- 仍提供 `WorkspaceContext`（workspace / workspaceMeta / authLogin）。
- **保留切换即 remount 行为**：把 `<Outlet />` 包一层 `key={selectedWorkspace}`，workspace 变化时子页面重新挂载（等价于现在 index route 的 key 技巧）。
- **空状态守卫**：`selectedWorkspace` 为空时渲染「请选择一个 workspace」提示（带打开选择器/添加 workspace 的入口），而不是让子页面拿空字符串去请求。

### 3. navigation.ts — 静态路径
- workspace-scoped 页面改为静态路径：`workspaces→/skills`、`publish→/publish`、`invitations→/members`、`activity→/activity`。
- `buildNavPath`：workspace 页直接返回静态路径（不再拼 owner/repo）。
- `routeToPage`：改为匹配 `/skills` `/publish` `/members` `/activity`。
- `workspaceFromPathname`：**删除**（无引用后）。`workspaceSubPath` 相应简化或删除。
- `navRoutes`：workspace 项改 `path` 为静态路径，去掉 `suffix`/`scope=workspace` 的特殊处理。

### 4. RootLayout.tsx — store 为真相源
- `workspaceRef` 改为 `useAppStore((s) => s.selectedWorkspace) ?? ""`，删掉 `workspaceFromPathname` + 上一轮的 URL→store 同步 effect。
- 4 处导航改为：`setSelectedWorkspace(ws)` + `navigate({ to: "/skills" })`（新增/deeplink 默认落到 skills 页；sidebar 选择同理）。
- `workspaceMeta` 用 `selectedWorkspace` 查找（已是此逻辑，沿用）。

### 5. Sidebar.tsx — 选择即设 store
- 删除 `workspaceFromPathname`；nav 链接用 `navigation.ts` 的静态路径。
- `onSelectWorkspace`：调用方（RootLayout）里 `setSelectedWorkspace` + 跳 `/skills`。
- 选择器 `current` 用 RootLayout 传入的 activeWorkspace（= selectedWorkspace）。

### 6. deep-link 处理（RootLayout confirmDeepLink）
- 解析出 ws 后 `setSelectedWorkspace(ws)` + `navigate({ to: "/skills" })`，替代旧的 `/workspace/$owner/$repo`。

---

## 边界与风险
- **per-workspace 的 localStorage 状态**（`ws-ui:${workspace}:...`）：key 里带 workspace 名，与路由无关，重构后照常工作。
- **remount 行为**：靠 WorkspaceShell 的 `key={selectedWorkspace}` 保留，切 workspace 仍重置页面内 state（与现状一致）。
- **空 workspace**：新增空状态守卫，比现状（空字符串发请求）更健壮。
- **深链/外部唤起**：deep-link 仍可携带 workspace（payload 里有），只是落地时写 store 而非 URL —— 行为不变。
- **可逆**：改动集中在 router + shell + navigation + 4 个 route 文件，纯前端，git 可回退。

## 验证
- `pnpm check`（类型）+ `pnpm build`（构建）。
- 你跑 `pnpm dev:web` 手测：选 workspace → 切 discover/my-skills → 选择器保持；点 skills/publish/members/activity 直达选中 workspace；切换 workspace 后页面状态重置；无 workspace 时显示空状态。

## 待批准后先做
按 1→6 顺序改，每步后 `pnpm check`。涉及文件：
`router.tsx`、`shell/WorkspaceShell.tsx`、`utils/navigation.ts`、`shell/RootLayout.tsx`、`shell/Sidebar.tsx`（4 个 route 组件基本不动，因为它们只读 `useWorkspace()`）。
