import type { ProviderInstance, StoredWorkspace, Workspace } from "./skill-library";

export function normalizeProviderId(value: string | null | undefined) {
  const trimmed = (value ?? "").trim().toLowerCase();
  if (!trimmed || trimmed === "github") return "github.com";
  return trimmed;
}

export function workspaceProviderId(workspace: string) {
  const parts = workspace.split("/").filter(Boolean);
  return parts.length > 2 ? normalizeProviderId(parts[0]) : "github.com";
}

export function githubRepoPath(workspace: string) {
  const parts = workspace.split("/").filter(Boolean);
  if (parts.length > 2 && normalizeProviderId(parts[0]) === "github.com") {
    return parts.slice(1).join("/");
  }
  return parts.join("/");
}

export function providerKindValue(instance: ProviderInstance | undefined) {
  if (!instance) return null;
  return typeof instance.kind === "string" ? instance.kind : instance.kind.custom;
}

export function providerSupportsComments(
  instance: ProviderInstance | undefined,
  providerId: string,
) {
  const kind = providerKindValue(instance);
  if (kind) {
    const normalized = kind.toLowerCase();
    return normalized === "github" || normalized === "git-hub";
  }
  const normalized = providerId.toLowerCase();
  return normalized === "github.com" || normalized === "github";
}

export function workspaceKey(workspace: Pick<Workspace, "provider" | "full_name">) {
  return `${normalizeProviderId(workspace.provider)}/${workspace.full_name}`;
}

export function workspaceInputForProvider(providerId: string, workspacePath: string) {
  const normalized = normalizeProviderId(providerId);
  const path = workspacePath.trim().replace(/^\/+|\/+$/g, "");
  if (!path) return "";
  const parts = path.split("/").filter(Boolean);
  if (parts[0] && normalizeProviderId(parts[0]) === normalized) {
    return parts.join("/");
  }
  return `${normalized}/${parts.join("/")}`;
}

export function workspaceMatchesSelection(
  workspace: Pick<StoredWorkspace, "provider" | "full_name">,
  selection: string | null | undefined,
) {
  const selected = (selection ?? "").trim();
  if (!selected) return false;
  if (selected === workspaceKey(workspace)) return true;
  return normalizeProviderId(workspace.provider) === "github.com" && selected === workspace.full_name;
}

export function workspaceProviderLabel(providerId: string | null | undefined) {
  const normalized = normalizeProviderId(providerId);
  if (normalized === "github.com") return "GitHub";
  if (normalized === "gitlab.com") return "GitLab";
  if (normalized === "gitee.com") return "Gitee";
  if (normalized.includes("webdav") || normalized.includes("web-dav")) return "WebDAV";
  return normalized;
}

export function workspaceProviderShortLabel(providerId: string | null | undefined) {
  const normalized = normalizeProviderId(providerId);
  if (normalized === "github.com") return "GH";
  if (normalized === "gitlab.com") return "GL";
  if (normalized === "gitee.com") return "GE";
  if (normalized.includes("webdav") || normalized.includes("web-dav")) return "DAV";
  return normalized.slice(0, 3).toUpperCase();
}

export function providerIsWebDav(instance: ProviderInstance | undefined) {
  const kind = providerKindValue(instance)?.toLowerCase();
  return kind === "web-dav" || kind === "webdav";
}
