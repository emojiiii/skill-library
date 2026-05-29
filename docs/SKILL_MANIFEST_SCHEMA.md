# Skill Manifest Schema 草案

> 状态：MVP 草案
> 机器可读版本：`docs/schemas/skill-manifest.schema.json`
> 关联文档：`PRODUCT_DOCUMENT.md` 第 5.3 / 7 / 20 节，`MVP_TASKS.md` M2-1

## 1. 设计目标

Skill manifest 是 Team AI Hub 识别、展示、订阅、同步和安装 Skill 的最小元数据契约。

MVP 采用 **"必填最小集 + 可选扩展字段"**：

- 必填字段足够少，保证现有 Skill 仓库能低成本接入。
- 核心字段严格校验，保证 Web UI、CLI、diff、Rust native installer 有稳定输入。
- 扩展字段允许透传，避免早期 schema 过度限制未来资产形态。
- manifest 不表达权限控制，权限仍完全继承 Git Provider。

## 2. 文件位置与优先级

一个 Skill 目录可用两种方式声明 manifest：

1. `manifest.yaml` / `manifest.yml` / `manifest.json`
2. `SKILL.md` frontmatter

如果两者同时存在，MVP 规则如下：

1. 独立 manifest 文件优先。
2. `SKILL.md` frontmatter 作为补充，只填补独立 manifest 缺失的可选字段。
3. 同一字段冲突时，Web UI 显示 warning，CLI 以独立 manifest 为准。

建议新项目使用独立 `manifest.yaml`，方便 schema 校验和版本 diff。

## 3. 最小示例

```yaml
schemaVersion: 1
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews code changes for correctness and security.
version: 1.4.2
targets:
  - claude-code
  - cursor
permissions:
  - filesystem.read
  - shell.execute.limited
```

## 4. 完整示例

```yaml
schemaVersion: 1
id: code-reviewer
type: skill
name: Code Reviewer
description: Reviews code changes for correctness, security, and maintainability.
version: 1.4.2
license: MIT
homepage: https://github.com/acme/team-skills/tree/main/code-reviewer
repository:
  provider: github
  owner: acme
  repo: team-skills
  path: code-reviewer
authors:
  - name: Alice Chen
    email: alice@example.com
targets:
  - claude-code
  - cursor
permissions:
  - filesystem.read
  - shell.execute.limited
tags:
  - code-review
  - security
runtime:
  claude-code:
    minVersion: 1.0.0
  cursor:
    mode: project-rule
compatibility:
  os:
    - darwin
    - linux
dependencies:
  tools:
    - name: git
      required: true
files:
  entry: SKILL.md
  include:
    - SKILL.md
    - scripts/**
    - templates/**
  exclude:
    - .DS_Store
risk:
  level: medium
  notes: Executes read-only git commands during review.
```

## 5. 字段定义

| 字段 | 必填 | 类型 | 说明 |
|---|---:|---|---|
| `schemaVersion` | 否 | integer | schema 主版本。MVP 默认 `1`，推荐显式填写。 |
| `id` | 是 | string | Skill 稳定 ID。目录内唯一，建议 kebab-case。 |
| `type` | 是 | enum | MVP 只能是 `skill`。 |
| `name` | 是 | string | UI 展示名称。 |
| `description` | 是 | string | 一句话描述，建议 20-160 字符。 |
| `version` | 是 | string | SemVer，不带 `v` 前缀；与 git tag `v{version}` 对齐。 |
| `targets` | 是 | string[] | 支持的 runtime。MVP：`claude-code`、`cursor`、`codex`。 |
| `permissions` | 否 | string[] | Skill 需要的能力声明。缺省为空数组。 |
| `tags` | 否 | string[] | 搜索与分类标签。 |
| `authors` | 否 | object[] | 作者列表。 |
| `license` | 否 | string | SPDX license id 或自定义文本。 |
| `homepage` | 否 | uri | 项目主页。 |
| `repository` | 否 | object | Provider 仓库定位信息。通常可由 workspace 推导。 |
| `runtime` | 否 | object | 各 runtime 的专属配置。 |
| `compatibility` | 否 | object | OS / 架构兼容性。 |
| `dependencies` | 否 | object | 外部工具或其他 Skill 依赖。 |
| `files` | 否 | object | 打包包含/排除规则。 |
| `risk` | 否 | object | 风险等级与说明，供安装确认 UI 使用。 |

## 6. ID 与版本规则

### 6.1 `id`

- 只能包含小写字母、数字、点、下划线、短横线。
- 长度 2-80。
- 在同一 workspace 内必须唯一。
- 一旦发布，不建议修改。改名应视为新 Skill。

推荐：

```yaml
id: code-reviewer
```

不推荐：

```yaml
id: "Code Reviewer"
```

### 6.2 `version`

- 使用 SemVer：`MAJOR.MINOR.PATCH`，可带 prerelease/build metadata。
- manifest 内不带 `v` 前缀：`1.4.2`。
- git tag 推荐带 `v` 前缀：`v1.4.2`。
- 如果 tag 与 manifest version 不一致，解析器必须给出 warning。CLI 安装以 git ref 为准，Web UI 同时展示两者差异。

## 7. Runtime Targets

MVP 内置 target：

| target | 说明 |
|---|---|
| `claude-code` | 安装到 Claude Code skill 目录。 |
| `cursor` | 安装到 Cursor skills 目录。 |
| `codex` | Phase 2，MVP 只展示兼容性，不安装。 |

允许自定义 target，格式为 reverse DNS 或 kebab-case：

```yaml
targets:
  - claude-code
  - com.acme.internal-agent
```

未知 target 的处理：

- Web UI：展示为 "Custom runtime"。
- CLI：Rust installer 不支持该 target 时跳过并提示；必要时走显式 fallback。
- sync：不影响其他 target 安装。

## 8. 权限声明

`permissions` 是安装风险提示，不是权限强制沙箱。MVP 只做声明、展示、确认和 diff 高亮。

内置权限建议：

| 权限 | 风险 | 说明 |
|---|---|---|
| `filesystem.read` | low | 读取本地文件。 |
| `filesystem.write` | high | 写入本地文件。 |
| `shell.execute.limited` | medium | 执行受限命令，如 `git diff`。 |
| `shell.execute` | high | 执行任意 shell 命令。 |
| `network.provider` | low | 调用 Git Provider API。 |
| `network.external` | high | 访问任意外部网络。 |
| `secrets.read` | critical | 读取密钥或凭据。MVP 默认阻断，除非用户显式允许。 |

安装确认规则：

- low：不打断，仅展示。
- medium：首次安装确认。
- high：首次安装和每次权限升级都确认。
- critical：默认拒绝，需要 CLI 参数或管理员策略放行。

## 9. Runtime 专属配置

`runtime` 是 target 专属配置容器。核心 schema 不强校验内部字段，只约束顶层是 object。

示例：

```yaml
runtime:
  claude-code:
    minVersion: 1.0.0
  cursor:
    mode: project-rule
    ruleName: Code Reviewer
```

MVP wrapper 规则：

- Rust native installer 尽量少解释 runtime 专属字段，只读取 MVP 所需配置。
- fallback 只在原生 installer 覆盖不到时读取对应 target 的配置。
- 未识别字段原样保留，供后续安装策略使用。

## 10. 文件打包规则

`files` 用于决定安装时包含哪些文件。MVP 可先实现默认规则，再逐步支持 include/exclude。

默认包含：

- `SKILL.md`
- `manifest.yaml` / `manifest.yml` / `manifest.json`
- `scripts/**`
- `templates/**`
- `examples/**`
- `assets/**`
- `CHANGELOG.md`
- `README.md`

默认排除：

- `.git/**`
- `node_modules/**`
- `.DS_Store`
- 临时文件与编辑器缓存

如果 manifest 声明了 `files.include`，以 include 为主，再应用 exclude。

## 11. 解析与校验策略

MVP 解析器输出三类结果：

```typescript
type ManifestParseResult =
  | { ok: true; manifest: SkillManifest; warnings: ManifestWarning[] }
  | { ok: false; errors: ManifestError[]; warnings: ManifestWarning[] };
```

### 11.1 Hard errors

这些错误会让 Skill 不被识别：

- 缺少 `id`、`type`、`name`、`description`、`version`、`targets`。
- `type` 不是 `skill`。
- `id` 格式非法。
- `version` 不是 SemVer。
- `targets` 为空。
- YAML / JSON 语法错误。

### 11.2 Warnings

这些问题不阻断识别，但 UI 和 CLI 应展示：

- manifest version 与 git tag 不一致。
- `permissions` 含未知权限。
- `targets` 含未知 runtime。
- `description` 过短或过长。
- 独立 manifest 与 frontmatter 字段冲突。
- `files.include` 匹配不到任何文件。

## 12. 语义 Diff 规则

语义 diff 基于规范化后的 manifest 计算，忽略字段顺序和 YAML/JSON 格式差异。

MVP 必须高亮：

- `version` 变化
- `permissions` 新增/删除，尤其高风险权限
- `targets` 新增/删除
- `dependencies` 新增/删除
- `files.include` / `files.exclude` 变化

输出建议：

```json
{
  "changes": [
    {
      "path": "permissions",
      "kind": "added",
      "value": "shell.execute.limited",
      "risk": "medium"
    }
  ]
}
```

## 13. JSON Schema 使用方式

机器可读 schema 位于：

```text
docs/schemas/skill-manifest.schema.json
```

建议实现：

- 开发期：用 schema 驱动编辑器提示。
- 运行期：Rust 用 serde + 自定义 validator 作为主校验器；前端表单可用 zod 做输入校验。
- 测试：用 JSON Schema 样例做 cross-check，避免文档和实现漂移。

实现里可以从 schema 生成 Rust / TypeScript 类型，也可以手写 serde struct + zod schema；关键是字段语义与本文档保持一致。

## 14. 后续演进

### v1.1

- 增加 `maintainers` 与 `support` 字段。
- 增加 `deprecation` 字段，用于提示 Skill 迁移。
- 增加 `signature` 字段，支持发布包签名。

### v2

- 支持多资产类型：`prompt`、`workflow`、`knowledge-pack`。
- 把 runtime 专属配置拆成独立 schema。
- 支持组织级策略覆盖，比如禁止 `shell.execute`。

## 15. 当前决策

MVP 先按本草案实现：

1. 必填最小集严格校验。
2. 可选扩展字段允许透传。
3. 权限只做声明和风险提示，不做沙箱承诺。
4. 独立 manifest 优先，frontmatter 作为兼容入口。
5. 语义 diff 只覆盖核心字段，后续扩展。
