# 方案：真·风格迁移到 HeroUI 原生外观

## 你的两个决定
1. **桥接 token + 卸覆盖**（最大力度，会真实改变全 app 外观）
2. **复用 HeroUI 明暗**（明暗/配色由 HeroUI 变量体系驱动）

## 问题根因（为什么"看起来没变"）
- **假迁移**：`Card.tsx` 用 `variant="transparent"` + 一堆 className（`border-[var(--line)]`/`bg-[var(--bg-elevated)]`/`shadow-none`/`rounded-[var(--radius)]`）把 HeroUI Card 的原生圆角+阴影**全覆盖回**旧扁平风格。组件换了，外观被强行拉回原样。
- **两套 token 割裂**：app 的 `--bg`/`--fg`/`--brand`/`--line`（被引用 400+ 次）与 HeroUI 的 `--surface`/`--foreground`/`--accent`/`--border` 之间**没有桥接**。

## 关键事实（已核实）
- `--radius`：app `8px` == HeroUI `0.5rem`，不冲突；HeroUI radius 体系 `--radius-xs..4xl` 都基于 `--radius` 计算。
- 明暗：app + HeroUI 都响应 `.dark` class，`useTheme()` 已驱动；天然兼容。
- 命名冲突 token：`--success`/`--warning`/`--danger`/`--success-soft`/`--warning-soft`/`--danger-soft` 两边都有（HeroUI 也提供 soft 变体）。
- app 独有、HeroUI 无对应：`--bg-sidebar`、`--code-bg`、`--code-fg`、`--shadow-*`、`--radius-lg`、`--sidebar-width`、`--info`、`--fg-disabled`、`--line-strong`、`--fg-secondary`。
- `Pill.tsx` 已是 HeroUI Chip 原生用法（color+variant），无需改。

---

## 实施步骤

### 步骤 1 — 主题层桥接（styles.css 顶部 :root / :root.dark / accent 块）
把 app token **重指向** HeroUI 语义 token（单处定义，明暗自动跟随 HeroUI）：

| app token | → HeroUI token |
|---|---|
| `--bg` | `var(--background)` |
| `--bg-elevated` | `var(--surface)` |
| `--bg-soft` | `var(--surface-secondary)` |
| `--bg-active` | `var(--surface-tertiary)` |
| `--bg-sidebar` | `var(--background-secondary)` |
| `--fg` | `var(--foreground)` |
| `--fg-secondary` | `var(--muted)` |
| `--fg-muted` | `var(--muted)` |
| `--line` | `var(--border)` |
| `--line-strong` | `var(--border-secondary)` |
| `--brand` | `var(--accent)` |
| `--brand-hover` | `var(--accent-hover)` |
| `--brand-soft` | `var(--accent-soft)` |
| `--brand-fg` | `var(--accent-soft-foreground)` |

- **冲突 token**（`--success`/`--warning`/`--danger` + soft）：删除 app 定义，让 HeroUI 的流过（app CSS 里的 `var(--success)` 自动解析到 HeroUI 值）。
- **保留** app 独有 token：`--code-bg`/`--code-fg`（代码面板恒暗）、`--shadow-*`、`--radius-lg`、`--sidebar-width`、`--info`、`--fg-disabled`。
- **`:root.dark` 颜色块大幅精简**：桥接后的 token 自动跟随 HeroUI 暗色，无需重复定义；只保留 app 独有 token 的暗色（若有）。
- **4 个 accent 块**：从覆盖 `--brand` 改为覆盖 `--accent`（HeroUI），下游 `--brand` 自动跟随。明暗下的 accent 仍由这些类控制。

### 步骤 2 — 卸掉 Card 的覆盖样式（Card.tsx）
- 去掉 `variant="transparent"` 和 `border/bg/shadow-none/rounded/p-0` 覆盖 → 用 HeroUI 默认 `Card`（原生圆角 `min(32px, --radius-3xl)` + `--surface-shadow` 阴影 + 内边距）。
- `Card.Title`→HeroUI Title、`Card.Subtitle`→Description、`Card.Body`→Content，去掉自定义字号/颜色覆盖（让 HeroUI 排版生效）。
- **唯一保留的最小覆盖**：`Card.Header` 原生是 `flex-direction:column`，而多处 header 是「标题 + 右侧操作按钮」的两栏布局 → 给 Header 加 `flex-row items-center justify-between` 防止布局塌成竖排。这是结构性必需，非样式美化。

### 步骤 3 — 验证
- `pnpm check`（类型）+ `pnpm build`（构建）——机械验证。
- **视觉需你确认**：我读不到运行中的 UI（截图工具在此环境失效），颜色/外观是否符合预期要你跑 `pnpm dev:web` 目测。
- 因为桥接全部集中在 styles.css 顶部一处，任何颜色不对都是**一行微调**，可快速迭代/回滚。

## 影响面与风险
- 全 app 外观会变：卡片变 HeroUI 圆角+柔和阴影；配色走 HeroUI oklch 体系（与现有 hex 略有色差）。
- 改动集中：styles.css 顶部主题块 + Card.tsx，**不逐个改 400+ 调用点**（它们继续用 `var(--bg)` 等，只是这些 token 现在指向 HeroUI）。
- 可逆：保留旧主题块的 git 历史；桥接是声明式集中改动，回退成本低。

## 待确认后先做
步骤 1（主题桥接）+ 步骤 2（卸 Card 覆盖），改动文件：`src/styles.css`、`src/widgets/Card.tsx`。
