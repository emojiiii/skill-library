import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./skill-library";

/**
 * A skill entry from the public skills.sh registry.
 *
 * `source` is a GitHub "owner/repo" that feeds the anonymous read path
 * (getSkillDetail / getWorkspaceDetail) — no GitHub token required.
 */
export interface RegistrySkill {
  /** Composite id: "owner/repo/skillId" */
  id: string;
  skillId: string;
  name: string;
  installs: number;
  source: string;
  isOfficial: boolean;
}

/**
 * Curated fallback list shown when the registry is unreachable (offline,
 * rate-limited) or before the user has typed a query. Keeps the discover
 * screen from ever being blank for a brand-new anonymous user.
 *
 * These all map to real public GitHub repos via `source` (verified against the
 * live skills.sh registry). Ordered roughly by popularity.
 */
export const FEATURED_SKILLS: RegistrySkill[] = [
  {
    id: "vercel-labs/skills/find-skills",
    skillId: "find-skills",
    name: "find-skills",
    installs: 1769412,
    source: "vercel-labs/skills",
    isOfficial: true,
  },
  {
    id: "anthropics/skills/frontend-design",
    skillId: "frontend-design",
    name: "frontend-design",
    installs: 479812,
    source: "anthropics/skills",
    isOfficial: true,
  },
  {
    id: "vercel-labs/agent-skills/vercel-react-best-practices",
    skillId: "vercel-react-best-practices",
    name: "vercel-react-best-practices",
    installs: 438407,
    source: "vercel-labs/agent-skills",
    isOfficial: true,
  },
  {
    id: "vercel-labs/agent-skills/web-design-guidelines",
    skillId: "web-design-guidelines",
    name: "web-design-guidelines",
    installs: 354049,
    source: "vercel-labs/agent-skills",
    isOfficial: true,
  },
  {
    id: "mattpocock/skills/improve-codebase-architecture",
    skillId: "improve-codebase-architecture",
    name: "improve-codebase-architecture",
    installs: 189875,
    source: "mattpocock/skills",
    isOfficial: false,
  },
  {
    id: "vercel-labs/agent-skills/vercel-composition-patterns",
    skillId: "vercel-composition-patterns",
    name: "vercel-composition-patterns",
    installs: 193250,
    source: "vercel-labs/agent-skills",
    isOfficial: true,
  },
  {
    id: "xixu-me/skills/github-actions-docs",
    skillId: "github-actions-docs",
    name: "github-actions-docs",
    installs: 180205,
    source: "xixu-me/skills",
    isOfficial: false,
  },
  {
    id: "mattpocock/skills/grill-with-docs",
    skillId: "grill-with-docs",
    name: "grill-with-docs",
    installs: 178955,
    source: "mattpocock/skills",
    isOfficial: false,
  },
  {
    id: "obra/superpowers/test-driven-development",
    skillId: "test-driven-development",
    name: "test-driven-development",
    installs: 105041,
    source: "obra/superpowers",
    isOfficial: false,
  },
  {
    id: "obra/superpowers/requesting-code-review",
    skillId: "requesting-code-review",
    name: "requesting-code-review",
    installs: 106597,
    source: "obra/superpowers",
    isOfficial: false,
  },
  {
    id: "anthropics/skills/webapp-testing",
    skillId: "webapp-testing",
    name: "webapp-testing",
    installs: 83848,
    source: "anthropics/skills",
    isOfficial: true,
  },
  {
    id: "anthropics/skills/canvas-design",
    skillId: "canvas-design",
    name: "canvas-design",
    installs: 64348,
    source: "anthropics/skills",
    isOfficial: true,
  },
];

/**
 * Curated discovery categories. The skills.sh API has no category/tag concept
 * (only fuzzy search), so each category is just a canned search query. Clicking
 * a category runs that query through the normal search path.
 *
 * `labelKey` resolves via the i18n t() helper at render time.
 */
export interface SkillCategory {
  id: string;
  labelKey: string;
  query: string;
}

export const SKILL_CATEGORIES: SkillCategory[] = [
  { id: "writing", labelKey: "discover.cat.writing", query: "writing" },
  { id: "design", labelKey: "discover.cat.design", query: "design" },
  { id: "frontend", labelKey: "discover.cat.frontend", query: "frontend" },
  { id: "data", labelKey: "discover.cat.data", query: "data" },
  { id: "research", labelKey: "discover.cat.research", query: "research" },
  { id: "marketing", labelKey: "discover.cat.marketing", query: "marketing" },
  { id: "video", labelKey: "discover.cat.video", query: "video" },
  { id: "pdf", labelKey: "discover.cat.pdf", query: "pdf" },
  { id: "finance", labelKey: "discover.cat.finance", query: "finance" },
  { id: "seo", labelKey: "discover.cat.seo", query: "seo" },
  { id: "review", labelKey: "discover.cat.review", query: "review" },
  { id: "test", labelKey: "discover.cat.test", query: "test" },
  { id: "docs", labelKey: "discover.cat.docs", query: "docs" },
  { id: "security", labelKey: "discover.cat.security", query: "security" },
];

/**
 * Search the public skill registry. Proxied through a Rust command because the
 * registry API does not send CORS headers, so the webview cannot fetch it
 * directly. Returns [] outside the desktop app or for queries shorter than 2
 * characters.
 */
export async function searchSkillsRegistry(query: string): Promise<RegistrySkill[]> {
  const needle = query.trim();
  if (!isTauri || needle.length < 2) return [];
  return invoke<RegistrySkill[]>("search_skills_registry", { query: needle });
}

/** Split a registry `source` ("owner/repo") into owner + repo parts. */
export function parseSource(source: string): { owner: string; repo: string } | null {
  const [owner, repo] = source.split("/");
  if (!owner || !repo) return null;
  return { owner, repo };
}
