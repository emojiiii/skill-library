import type { Workspace } from "../lib/skill-library";

const palette = [
  ["#eef2ff", "#312e81"],
  ["#fef3c7", "#92400e"],
  ["#dcfce7", "#166534"],
  ["#fae8ff", "#86198f"],
  ["#ffe4e6", "#9f1239"],
  ["#e0f2fe", "#075985"],
];

function hash(input: string): number {
  let h = 0;
  for (let i = 0; i < input.length; i += 1) {
    h = (h * 31 + input.charCodeAt(i)) | 0;
  }
  return Math.abs(h);
}

export function workspaceColor(name: string): { bg: string; fg: string } {
  const [bg, fg] = palette[hash(name) % palette.length];
  return { bg, fg };
}

export function workspaceInitials(workspace: { owner: string; repo: string } | Workspace): string {
  const owner = "owner" in workspace ? workspace.owner : "";
  const repo = "repo" in workspace ? workspace.repo : "";
  const o = owner.replace(/[^a-z0-9]/gi, "")[0] ?? "?";
  const r = repo.replace(/[^a-z0-9]/gi, "")[0] ?? "?";
  return `${o}${r}`.toUpperCase();
}
