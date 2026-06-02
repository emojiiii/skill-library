# 方案：修复源码变更红点 bug + 新增 AI 风险审查

## Part A — 修复"红点不消失" bug（小、独立、先做）

### 根因（已确认）
`SkillDetail.tsx:184` 的 dirty 判断：
```ts
hasUnpublishedChanges = editedContent !== null && editedContent.trim() !== sourceContent.trim()
```
- markdown 文件走 MDXEditor，它在 onChange 时吐出的是**规范化后**的 markdown（列表符号、标题空格、空行等被重排）。
- `sourceContent` 是 GitHub 原始 markdown。
- 你"加一段再删掉"后，编辑器内容 = 规范化版，原始 = 未规范化版 → `trim()` 只去首尾空白，治不了**内部**规范化差异 → 两者永不相等 → 红点常驻。
- 代码注释里其实已经写了"MDXEditor normalizes whitespace…trim both sides"，说明作者意识到了，但 trim 不够。

### 修复思路
真正的 dirty 应该是"**相对编辑器自己的初始规范化基线**有没有变"，而不是跟 GitHub 原始文本比。做法：
- 在 `MarkdownEditor` 首次规范化完成后，捕获一个 **baseline**（编辑器规范化后的初始内容），onChange 时跟 baseline 比，把"是否真的被用户改过"作为一个布尔值上报，而不是让父组件拿规范化文本去跟原始文本比。
- 具体：`MarkdownEditor` 已有 `userEditedRef`（onInput 才置 true）。再加一个 `baselineRef`：在 `setMarkdown(initialValue)` 之后的首个 onChange 里记下规范化基线；之后每次 onChange 用 `val !== baseline` 算 dirty，通过新增的 `onDirtyChange?(dirty: boolean)` 回调上报。
- `SkillDetail` 改为：markdown 模式下 dirty 以编辑器上报为准；非 markdown（CodeEditor，所见即所得、不规范化）仍用 `editedContent !== null && editedContent !== sourceContent`（去掉 trim 噪音，直接精确比较）。
- 删除/revert 时编辑器 setMarkdown 回 initialValue → baseline 重置 → dirty 归 false → 红点消失。

### 影响文件
`widgets/MarkdownEditor.tsx`（加 baseline + onDirtyChange）、`widgets/SkillDetail.tsx`（dirty 判定改用编辑器上报 + CodeEditor 精确比较）。

### 验证
手测：打开 SKILL.md → 加一段文字（红点亮）→ 删回去（红点灭）。CodeEditor 文件同理。

---

## Part B — AI 风险审查（新功能）

### 目标
用户在设置里配置一个 LLM provider（OpenAI / Anthropic 协议），填 BaseURL + API key；SkillRiskPanel 的 "AI review" 按钮启用，点击后把 SKILL.md 正文发给 LLM，让它审查"正文指令里有没有诱导危险操作"（弥补"只看 manifest 权限"的纸面安全）。

### B1. 设置层（SettingsDialog + AppSettings）
- `AppSettings` 加字段：`aiProvider: "openai" | "anthropic" | "none"`、`aiBaseUrl: string`、`aiModel: string`。**API key 不进 AppSettings/localStorage**（敏感）。
- 新增设置分区 "AI 审查"（Sparkles 图标）：provider 下拉、BaseURL 输入、model 输入、API key 输入（password）+ 保存/清除按钮、"测试连接"按钮。
- API key 走 **Rust keyring**（复用 skill-library-core 的 keyring 机制，新增一个 service/username 条目），不明文存盘。新增命令 `save_ai_credential` / `load_ai_credential(只返回是否已设置)` / `delete_ai_credential`。

### B2. 后端 provider 抽象（Rust）
- 新建 `ai_review` 模块（在 desktop src-tauri 内）。
- 一个 `review_skill` Tauri 命令：入参 `{ skillName, skillMarkdown, manifest 权限摘要 }`，读取设置 + keyring 里的 key，按 provider 协议拼请求：
  - **openai**: `POST {baseUrl}/chat/completions`，`Authorization: Bearer`
  - **anthropic**: `POST {baseUrl}/v1/messages`，`x-api-key` + `anthropic-version`
- 用既有 reqwest 客户端（外部 API 必须走后端，webview 无 CORS——与 registry 同理）。
- 统一的审查 prompt：要求 LLM 返回结构化 JSON `{ verdict: "safe"|"caution"|"danger", summary, findings: [{severity, detail}] }`，后端解析后回前端。
- 错误处理：未配置 provider/key → 返回明确错误码；网络/解析失败 → 友好提示。

### B3. 前端接线（SkillRiskPanel）
- "AI review" 按钮：未配置 provider 时显示"去设置配置"，已配置时显示"运行审查"。
- 点击 → 调 `review_skill`（用 SkillDetail 已有的 `skill_markdown.content` + manifest）→ 展示 verdict 徽章 + summary + findings 列表。
- 用 react-query mutation，带 loading/error 态。

### B4. SkillRiskPanel 需要 markdown 正文
- 现在 SkillRiskPanel 只收 `manifest` + `skillPath`。需让 SkillDetail 把 `skillMarkdown`（已有 `detail?.skill_markdown?.content`）传进来。

### 影响文件
- 前端：`shell/SettingsDialog.tsx`（AI 分区）、`widgets/SkillRiskPanel.tsx`（审查 UI）、`widgets/SkillDetail.tsx`（传 markdown）、`lib/skill-library.ts`（新命令封装）、`hooks/useLocale.ts`（文案）
- 后端：`src-tauri/src/lib.rs`（review_skill + ai credential 命令 + 注册）、可能新增 `src-tauri/src/ai_review.rs`
- 可能：`skill-library-core`（如果 keyring 条目复用需要小改）

### 待你确认的点（实施前）—— 已确认
1. provider 协议：**OpenAI + Anthropic** 两种（先不做 Gemini）。OpenAI 协议最通用，第三方网关/中转/本地模型多兼容它。
2. API key 存储：**OS keyring**（复用 skill-library-core 的 keyring，与 GitHub token 同机制，不落明文）。
3. 审查触发：**手动点按钮**（省 token、可控）。

---

## 执行顺序
1. **Part A 先做**（独立 bug 修复，低风险，立即可验证）
2. Part A 验证通过后，再做 Part B（先后端 provider + 命令，再设置 UI，最后 SkillRiskPanel 接线）

## 验证
- Part A：`pnpm check` + 手测红点
- Part B：`cargo check`/`cargo test`（后端）+ `pnpm check`/`build`（前端）+ 手测：配 provider → 跑审查 → 看结果
