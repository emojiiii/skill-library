import type { SkillManifest } from "../lib/teamai";

export const riskOrder = ["low", "medium", "high", "critical"] as const;
export type RiskRank = (typeof riskOrder)[number];

export const riskLabel: Record<string, string> = {
  low: "Low",
  medium: "Medium",
  high: "High",
  critical: "Critical",
};

export const riskTone: Record<string, "success" | "warning" | "danger" | "default"> = {
  low: "success",
  medium: "warning",
  high: "danger",
  critical: "danger",
};

export const permissionRisk: Record<string, RiskRank> = {
  "filesystem.write": "high",
  "shell.execute.limited": "medium",
  "shell.execute": "high",
  "network.external": "high",
  "secrets.read": "critical",
};

export function maxRisk(left: RiskRank, right: RiskRank): RiskRank {
  return riskOrder.indexOf(left) >= riskOrder.indexOf(right) ? left : right;
}

export function effectiveRisk(manifest: SkillManifest): RiskRank {
  return manifest.permissions.reduce<RiskRank>(
    (risk, permission) => maxRisk(risk, permissionRisk[permission] ?? ("low" as RiskRank)),
    (manifest.risk?.level ?? "low") as RiskRank,
  );
}

export function riskRequiresConfirmation(risk: string): boolean {
  return risk === "medium" || risk === "high" || risk === "critical";
}

export function permissionSummary(manifest: SkillManifest): string {
  return manifest.permissions.length ? manifest.permissions.join(", ") : "No permissions declared";
}

export const stateTone: Record<string, "success" | "warning" | "danger" | "default"> = {
  open: "warning",
  waiting_review: "warning",
  merged: "success",
  accepted: "success",
  pending: "warning",
  closed: "default",
  rejected: "danger",
  declined: "danger",
  expired: "default",
};

// ---------------------------------------------------------------------------
// Consumer-facing risk language.
//
// Non-technical users should never see "filesystem.write" or "critical risk".
// These helpers translate the developer risk model into plain-language safety
// signals. They build on the same effectiveRisk()/permissions data so there is
// a single source of truth — only the wording differs.
// ---------------------------------------------------------------------------

export type SafetyLevel = "safe" | "caution" | "sensitive";

/** Map the four-level developer risk to a three-level consumer safety signal. */
export function safetyLevel(manifest: SkillManifest): SafetyLevel {
  const risk = effectiveRisk(manifest);
  if (risk === "low") return "safe";
  if (risk === "medium") return "caution";
  return "sensitive"; // high | critical
}

export const safetyTone: Record<SafetyLevel, "success" | "warning" | "danger"> = {
  safe: "success",
  caution: "warning",
  sensitive: "danger",
};

/**
 * Plain-language capability lines derived from declared permissions, e.g.
 * "Can modify files on your computer". Returns an empty array when the skill
 * only reads — the caller can then show a reassuring "read-only" message.
 */
export function plainPermissionLines(
  manifest: SkillManifest,
  t: (key: string) => string,
): string[] {
  const lines: string[] = [];
  const perms = new Set(manifest.permissions);
  if (perms.has("filesystem.write")) lines.push(t("safety.cap.writeFiles"));
  if (perms.has("shell.execute") || perms.has("shell.execute.limited")) {
    lines.push(t("safety.cap.runScripts"));
  }
  if (perms.has("network.external")) lines.push(t("safety.cap.network"));
  if (perms.has("secrets.read")) lines.push(t("safety.cap.secrets"));
  return lines;
}

