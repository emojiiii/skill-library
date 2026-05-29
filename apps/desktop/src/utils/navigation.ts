export type AppPage =
  | "workspaces"
  | "installed"
  | "publish"
  | "invitations"
  | "subscriptions"
  | "activity"
  | "cli";

/** Pages that live under /workspace/$owner/$repo/... */
export const workspaceScopedPages: AppPage[] = ["workspaces", "publish", "invitations", "activity"];

/** Pages that are top-level (personal, not workspace-scoped) */
export const personalPages: AppPage[] = ["subscriptions", "installed", "cli"];

/** i18n keys for page title/subtitle — resolve via t() at render time */
export const pageCopyKeys: Record<AppPage, { titleKey: string; subtitleKey: string }> = {
  workspaces: { titleKey: "page.skills.title", subtitleKey: "page.skills.subtitle" },
  publish: { titleKey: "page.publish.title", subtitleKey: "page.publish.subtitle" },
  invitations: { titleKey: "page.members.title", subtitleKey: "page.members.subtitle" },
  subscriptions: { titleKey: "page.subscriptions.title", subtitleKey: "page.subscriptions.subtitle" },
  installed: { titleKey: "page.installed.title", subtitleKey: "page.installed.subtitle" },
  activity: { titleKey: "page.activity.title", subtitleKey: "page.activity.subtitle" },
  cli: { titleKey: "page.cli.title", subtitleKey: "page.cli.subtitle" },
};

/** @deprecated Use pageCopyKeys + t() instead. Kept for backward compat during migration. */
export const pageCopy: Record<AppPage, { title: string; subtitle: string }> = {
  workspaces: {
    title: "Skills",
    subtitle: "Browse, subscribe, and inspect skills in the active workspace.",
  },
  publish: {
    title: "Publish",
    subtitle: "Track publish PRs, policy checks, review gates, and auto-merge outcomes.",
  },
  invitations: {
    title: "Members",
    subtitle: "Invite collaborators and complete onboarding.",
  },
  subscriptions: {
    title: "Subscriptions",
    subtitle: "Review your subscription declarations and update strategy.",
  },
  installed: {
    title: "Local skills",
    subtitle: "Toggle which runtimes use each skill, push local skills to a team workspace.",
  },
  activity: {
    title: "Activity",
    subtitle: "Review provider webhook updates and sync polling inputs.",
  },
  cli: {
    title: "CLI",
    subtitle: "Run local-first Team AI Hub workflows from the Rust CLI.",
  },
};

/** Workspace-scoped sub-path suffix (empty string = skills index) */
export const workspaceSubPath: Record<string, AppPage> = {
  "": "workspaces",
  publish: "publish",
  invitations: "invitations",
  activity: "activity",
};

export interface NavRoute {
  page: AppPage;
  label: string;
  /** For personal pages, a static path. For workspace pages, a suffix appended to /workspace/$owner/$repo */
  scope: "workspace" | "personal";
  /** Static path for personal pages */
  path?: string;
  /** Suffix for workspace-scoped pages (empty = index) */
  suffix?: string;
}

export const navRoutes: NavRoute[] = [
  { page: "workspaces", label: "Skills", scope: "workspace", suffix: "" },
  { page: "publish", label: "Publish PRs", scope: "workspace", suffix: "publish" },
  { page: "invitations", label: "Members", scope: "workspace", suffix: "invitations" },
  { page: "activity", label: "Activity", scope: "workspace", suffix: "activity" },
  { page: "subscriptions", label: "Subscriptions", scope: "personal", path: "/subscriptions" },
  { page: "installed", label: "Local", scope: "personal", path: "/installed" },
  { page: "cli", label: "CLI", scope: "personal", path: "/cli" },
];

/** Build the full path for a nav route given the current workspace */
export function buildNavPath(route: NavRoute, workspace: string | null): string {
  if (route.scope === "personal") return route.path!;
  if (!workspace) return "/";
  const [owner, repo] = workspace.split("/");
  const base = `/workspace/${owner}/${repo}`;
  return route.suffix ? `${base}/${route.suffix}` : base;
}

export function routeToPage(pathname: string): AppPage {
  if (pathname.startsWith("/installed")) return "installed";
  if (pathname.startsWith("/subscriptions")) return "subscriptions";
  if (pathname.startsWith("/cli")) return "cli";
  // Workspace-scoped pages: /workspace/$owner/$repo/suffix
  const wsMatch = pathname.match(/^\/workspace\/[^/]+\/[^/]+(?:\/(.*))?$/);
  if (wsMatch) {
    const suffix = wsMatch[1] ?? "";
    return workspaceSubPath[suffix] ?? "workspaces";
  }
  return "workspaces";
}

/** Extract workspace full_name from a /workspace/$owner/$repo path. Returns null if not a workspace route. */
export function workspaceFromPathname(pathname: string): string | null {
  const match = pathname.match(/^\/workspace\/([^/]+)\/([^/]+)/);
  if (match) return `${match[1]}/${match[2]}`;
  return null;
}

export const inviteRoles = ["read", "triage", "write", "maintain", "admin"] as const;
export type InviteRole = (typeof inviteRoles)[number];

export const inviteRoleLabel: Record<InviteRole, string> = {
  read: "Read",
  triage: "Triage",
  write: "Write",
  maintain: "Maintain",
  admin: "Admin",
};
