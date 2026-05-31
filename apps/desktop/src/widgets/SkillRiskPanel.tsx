import { Button, Spinner } from "@heroui/react";
import { useMutation } from "@tanstack/react-query";
import { AlertTriangle, ShieldCheck, Sparkles } from "lucide-react";
import type { AiReviewResult, SkillManifest } from "../lib/teamai";
import { reviewSkill } from "../lib/teamai";
import { useLocale } from "../hooks/useLocale";
import { useTheme } from "../hooks/useTheme";
import { effectiveRisk, riskLabel } from "../utils/risk";
import { formatError } from "../utils/format";
import { Card } from "./Card";
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

const verdictTone: Record<string, PillTone> = {
  safe: "success",
  caution: "warning",
  danger: "danger",
};

const aiSeverityTone: Record<string, PillTone> = {
  info: "default",
  warning: "warning",
  danger: "danger",
};

export function SkillRiskPanel({
  manifest,
  skillPath,
  workspace,
  refName,
}: {
  manifest: SkillManifest;
  skillPath: string;
  workspace: string;
  refName?: string;
}) {
  const { t } = useLocale();
  const settings = useTheme();
  const risk = effectiveRisk(manifest);
  const findings = analyzeManifest(manifest, t);

  const dangerCount = findings.filter((f) => f.level === "danger").length;
  const warningCount = findings.filter((f) => f.level === "warning").length;

  const aiConfigured = settings.aiProvider !== "none" && Boolean(settings.aiBaseUrl);

  const review = useMutation<AiReviewResult, Error>({
    mutationFn: () =>
      reviewSkill({
        provider: settings.aiProvider,
        baseUrl: settings.aiBaseUrl,
        model: settings.aiModel,
        workspace,
        skillPath,
        refName,
        skillName: manifest.name,
        permissions: manifest.permissions,
      }),
  });

  return (
    <div className="space-y-4">
      {/* Headline */}
      <Card className="p-4">
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
      </Card>

      {/* AI review */}
      <Card className="p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-[13px] font-semibold">
              <Sparkles size={14} className="text-[var(--brand)]" />
              {t("risk.aiReview")}
            </div>
            <p className="mt-1 text-[12px] text-[var(--fg-muted)]">
              {aiConfigured ? t("risk.aiReviewDesc") : t("risk.aiNotConfigured")}
            </p>
          </div>
          <Button
            size="sm"
            variant={aiConfigured ? "secondary" : "outline"}
            isDisabled={!aiConfigured || review.isPending}
            onPress={() => review.mutate()}
          >
            {review.isPending ? <Spinner size="sm" /> : null}
            {review.isPending ? t("risk.aiReviewing") : t("risk.aiRunReview")}
          </Button>
        </div>

        {review.error ? (
          <div className="mt-3 rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
            {formatError(review.error)}
          </div>
        ) : null}

        {review.data ? (
          <div className="mt-3 space-y-2 border-t border-[var(--line)] pt-3">
            <div className="flex items-center gap-2">
              <Pill tone={verdictTone[review.data.verdict] ?? "default"}>
                {t(`risk.verdict.${review.data.verdict}`)}
              </Pill>
              <span className="text-[12px] text-[var(--fg-secondary)]">{review.data.summary}</span>
            </div>
            {review.data.findings.length ? (
              <div className="space-y-1.5">
                {review.data.findings.map((f, i) => (
                  <div key={i} className="flex items-start gap-2 text-[12px]">
                    <Pill tone={aiSeverityTone[f.severity] ?? "default"}>{f.severity}</Pill>
                    <span className="text-[var(--fg-secondary)]">{f.detail}</span>
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        ) : null}
      </Card>

      {/* Static findings */}
      <Card className="overflow-hidden p-0 gap-0">
        <Card.Header>
          <Card.Title>{t("risk.staticAnalysis")}</Card.Title>
          <Pill>{findings.length} {findings.length === 1 ? t("risk.finding") : t("risk.findings")}</Pill>
        </Card.Header>
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
      </Card>
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
