import { Button } from "@heroui/react";
import { AlertTriangle, ShieldCheck, Sparkles } from "lucide-react";
import type { SkillManifest } from "../lib/teamai";
import { useLocale } from "../hooks/useLocale";
import { effectiveRisk, riskLabel } from "../utils/risk";
import { Pill, type PillTone } from "./Pill";

interface RiskFinding {
  level: "info" | "warning" | "danger";
  title: string;
  detail: string;
}

const dangerousPermissions = new Set([
  "shell.execute",
  "filesystem.write",
  "network.external",
  "secrets.read",
]);

const mediumPermissions = new Set(["shell.execute.limited"]);

export function SkillRiskPanel({
  manifest,
  skillPath,
}: {
  manifest: SkillManifest;
  skillPath: string;
}) {
  const { t } = useLocale();
  const risk = effectiveRisk(manifest);
  const findings = analyzeManifest(manifest, t);

  const dangerCount = findings.filter((f) => f.level === "danger").length;
  const warningCount = findings.filter((f) => f.level === "warning").length;

  return (
    <div className="space-y-4">
      {/* Headline */}
      <div className="card p-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="text-[10.5px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
              {t("risk.overallRisk")}
            </div>
            <div className="mt-1 flex items-center gap-2">
              <span className="text-[20px] font-semibold tracking-tight">
                {riskLabel[risk]}
              </span>
              {dangerCount ? <Pill tone="danger">{dangerCount} {t("risk.danger")}</Pill> : null}
              {warningCount ? <Pill tone="warning">{warningCount} {t("risk.warning")}</Pill> : null}
              {!dangerCount && !warningCount ? <Pill tone="success">{t("risk.clean")}</Pill> : null}
            </div>
            <div className="mt-1 truncate text-[11.5px] font-mono text-[var(--fg-muted)]">
              {skillPath}
            </div>
          </div>
        </div>
      </div>

      {/* AI review (stub) */}
      <div className="card p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="flex items-center gap-2 text-[13px] font-semibold">
              <Sparkles size={14} className="text-[var(--brand)]" />
              {t("risk.aiReview")}
            </div>
            <p className="mt-1 text-[12px] text-[var(--fg-muted)]">
              {t("risk.aiReviewDesc")}
            </p>
          </div>
          <Button size="sm" variant="outline" isDisabled>
            {t("risk.comingSoon")}
          </Button>
        </div>
      </div>

      {/* Static findings */}
      <div className="card overflow-hidden">
        <div className="card-header">
          <div className="card-title">{t("risk.staticAnalysis")}</div>
          <Pill>{findings.length} {findings.length === 1 ? t("risk.finding") : t("risk.findings")}</Pill>
        </div>
        {findings.length ? (
          <div className="divide-y divide-[var(--line)]">
            {findings.map((finding, i) => (
              <FindingRow key={`${finding.title}:${i}`} finding={finding} />
            ))}
          </div>
        ) : (
          <div className="empty-state">
            <ShieldCheck size={20} className="text-[var(--success)]" />
            <div className="empty-state__title">{t("risk.noIssues")}</div>
            <div>{t("risk.noIssuesDesc")}</div>
          </div>
        )}
      </div>
    </div>
  );
}

function FindingRow({ finding }: { finding: RiskFinding }) {
  const tone: PillTone =
    finding.level === "danger" ? "danger" : finding.level === "warning" ? "warning" : "default";
  const icon =
    finding.level === "danger" ? (
      <AlertTriangle size={13} className="text-[var(--danger)]" />
    ) : finding.level === "warning" ? (
      <AlertTriangle size={13} className="text-[var(--warning)]" />
    ) : (
      <ShieldCheck size={13} className="text-[var(--success)]" />
    );
  return (
    <div className="card-row">
      <div className="flex min-w-0 items-start gap-2">
        {icon}
        <div className="min-w-0">
          <div className="text-[12.5px] font-medium">{finding.title}</div>
          <div className="mt-0.5 text-[11.5px] text-[var(--fg-muted)]">{finding.detail}</div>
        </div>
      </div>
      <Pill tone={tone}>{finding.level}</Pill>
    </div>
  );
}

function analyzeManifest(manifest: SkillManifest, t: (key: string) => string): RiskFinding[] {
  const findings: RiskFinding[] = [];

  for (const permission of manifest.permissions) {
    if (dangerousPermissions.has(permission)) {
      findings.push({
        level: "danger",
        title: t("risk.dangerousPerm").replace("{perm}", permission),
        detail: t("risk.dangerousPerm.detail"),
      });
    } else if (mediumPermissions.has(permission)) {
      findings.push({
        level: "warning",
        title: t("risk.elevatedPerm").replace("{perm}", permission),
        detail: t("risk.elevatedPerm.detail"),
      });
    }
  }

  if (!manifest.targets.length) {
    findings.push({
      level: "warning",
      title: t("risk.noTargets"),
      detail: t("risk.noTargets.detail"),
    });
  }

  if (manifest.description.trim().length < 20) {
    findings.push({
      level: "info",
      title: t("risk.shortDesc"),
      detail: t("risk.shortDesc.detail"),
    });
  }

  if (manifest.risk?.level && manifest.risk.level !== "low") {
    findings.push({
      level: manifest.risk.level === "critical" ? "danger" : "warning",
      title: t("risk.authorDeclared").replace("{level}", manifest.risk.level),
      detail: manifest.risk.notes ?? t("risk.authorDeclared.detail"),
    });
  }

  return findings;
}
