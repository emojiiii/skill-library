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
