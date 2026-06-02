import { Button, Spinner } from "@heroui/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { AlertTriangle, CheckCircle2, Info, RefreshCw, Sparkles, UploadCloud } from "lucide-react";
import {
  commitReviewToRepo,
  getAuthStatus,
  getRemoteReview,
  getSkillContentHash,
  reviewSkill,
  type AiReviewFinding,
  type AiReviewResult,
  type SkillManifest,
} from "../lib/skill-library";
import { getReviewCache, putReviewCache, type ReviewCacheEntry } from "../lib/workspaceCache";
import {
  formatRelativeTime,
  normalizeRemoteReview,
  reviewVerdictMapKey,
  stringifyRemoteReview,
  type ReviewVerdictMap,
} from "../lib/review";
import { useLocale } from "../hooks/useLocale";
import { useTheme } from "../hooks/useTheme";
import { effectiveRisk } from "../utils/risk";
import { formatError } from "../utils/format";
import { Card } from "./Card";
import { Pill, type PillTone } from "./Pill";

type ReviewSource = "local" | "remote";

type DisplayReview = ReviewCacheEntry & {
  source: ReviewSource;
};

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

const writePermissions = new Set(["admin", "maintain", "write"]);

export function SkillRiskPanel({
  manifest,
  skillPath,
  workspace,
  refName,
  workspacePermission,
}: {
  manifest: SkillManifest;
  skillPath: string;
  workspace: string;
  refName?: string;
  workspacePermission?: string;
}) {
  const { t, locale } = useLocale();
  const settings = useTheme();
  const queryClient = useQueryClient();
  const risk = effectiveRisk(manifest);

  const activeAiConfig = settings.aiProvider !== "none" ? settings.aiConfigs?.[settings.aiProvider] : null;
  const aiConfigured = settings.aiProvider !== "none" && Boolean(activeAiConfig?.baseUrl);
  const canWrite = workspacePermission ? writePermissions.has(workspacePermission) : false;

  const hashKey = ["skill-content-hash", workspace, skillPath, refName] as const;
  const localReviewKey = ["review-cache", workspace, skillPath] as const;

  // Source of truth for the panel: the local SQLite cache, warmed (and seeded
  // into the query cache) by the workspace-load batch prefetch. Reading it is
  // local and effectively instant, so opening the panel triggers no network
  // round-trip and no spinner — the review is just a repo JSON we already have.
  const localReview = useQuery({
    queryKey: localReviewKey,
    queryFn: () => getReviewCache(workspace, skillPath),
    staleTime: 5_000,
  });

  // Background staleness probe. getSkillContentHash downloads + hashes the
  // skill, so it is the one genuinely expensive call here — it must NEVER gate
  // the review display. It runs only when a cached review exists to compare
  // against, and only surfaces a subtle "may be outdated" hint once resolved.
  const hashQuery = useQuery({
    queryKey: hashKey,
    queryFn: () => getSkillContentHash({ workspace, skillPath, refName }),
    enabled: Boolean(localReview.data),
    staleTime: 5 * 60_000,
    retry: false,
  });

  const syncReview = useMutation<ReviewCacheEntry, Error, ReviewCacheEntry>({
    mutationFn: async (entry) => {
      const reviewJson = stringifyRemoteReview(entry);
      await commitReviewToRepo({
        workspace,
        skillId: manifest.id,
        reviewJson,
      });
      const synced = { ...entry, synced: true };
      await putReviewCache(workspace, skillPath, synced);
      return synced;
    },
    onSuccess: (synced) => {
      queryClient.setQueryData(localReviewKey, synced);
      updateVerdictMap(synced.verdict);
    },
  });

  // Reflect a verdict into the workspace-wide map that drives the list badge,
  // so a fresh review lights up the "reviewed safe" marker without a reload.
  const updateVerdictMap = useCallback(
    (verdict: ReviewCacheEntry["verdict"]) => {
      queryClient.setQueryData<ReviewVerdictMap>(reviewVerdictMapKey(workspace), (prev) => ({
        ...(prev ?? {}),
        [manifest.id]: verdict,
      }));
    },
    [queryClient, workspace, manifest.id],
  );

  const review = useMutation<AiReviewResult, Error>({
    mutationFn: () =>
      reviewSkill({
        provider: settings.aiProvider,
        baseUrl: activeAiConfig?.baseUrl ?? "",
        model: activeAiConfig?.model ?? "",
        workspace,
        skillPath,
        refName,
        skillName: manifest.name,
        permissions: manifest.permissions,
        language: locale === "zh" ? "zh-CN" : "en",
      }),
    onSuccess: async (result) => {
      const auth = await getAuthStatus().catch(() => null);
      const entry: ReviewCacheEntry = {
        verdict: result.verdict,
        summary: result.summary,
        findings: result.findings,
        contentHash: result.contentHash,
        reviewedAt: new Date().toISOString(),
        reviewedBy: auth?.githubLogin ? `@${auth.githubLogin}` : null,
        model: activeAiConfig?.model ?? "",
        synced: false,
      };
      await putReviewCache(workspace, skillPath, entry);
      queryClient.setQueryData(hashKey, result.contentHash);
      queryClient.setQueryData(localReviewKey, entry);
      updateVerdictMap(entry.verdict);
      if (canWrite) {
        syncReview.mutate(entry);
      }
    },
  });

  // The cached review (warmed by the workspace-load prefetch, or written by a
  // local run) is what we display — immediately, no spinner. Source is "remote"
  // when it came from the shared repo (synced), "local" otherwise.
  const cachedEntry = localReview.data ?? null;
  const displayReview: DisplayReview | null = cachedEntry
    ? { ...cachedEntry, source: cachedEntry.synced ? "remote" : "local" }
    : null;

  // Staleness is advisory only: once the background hash probe resolves, we know
  // whether the skill changed since the cached review. Until then (or if the
  // probe fails) we optimistically treat the review as current — never block.
  const currentHash = hashQuery.data;
  const maybeOutdated = Boolean(
    displayReview && currentHash && displayReview.contentHash !== currentHash,
  );
  const needsRemoteSync =
    canWrite &&
    displayReview != null &&
    !displayReview.synced &&
    !syncReview.isPending;

  // Re-review = pull latest first, then decide. Someone else may have already
  // reviewed the current content and pushed it to the repo; in that case adopt
  // their result instead of spending tokens. Only run the model if no fresh
  // remote review covers the current hash.
  const [refreshing, setRefreshing] = useState(false);
  const handleRunReview = useCallback(async () => {
    setRefreshing(true);
    try {
      const [hashResult, rawRemote] = await Promise.all([
        getSkillContentHash({ workspace, skillPath, refName }).catch(() => null),
        getRemoteReview({ workspace, skillId: manifest.id }).catch(() => null),
      ]);
      if (hashResult) queryClient.setQueryData(hashKey, hashResult);
      const remoteResult = rawRemote ? normalizeRemoteReview(rawRemote) : null;
      if (remoteResult && hashResult && remoteResult.contentHash === hashResult) {
        // A fresh shared review already exists — adopt it, skip the model call.
        await putReviewCache(workspace, skillPath, remoteResult);
        queryClient.setQueryData(localReviewKey, remoteResult);
        updateVerdictMap(remoteResult.verdict);
        return;
      }
    } finally {
      setRefreshing(false);
    }
    review.mutate();
  }, [
    queryClient,
    hashKey,
    localReviewKey,
    workspace,
    skillPath,
    refName,
    manifest.id,
    review,
    updateVerdictMap,
  ]);
  const reviewBusy = refreshing || review.isPending;

  return (
    <div className="space-y-4">
      <Card className="p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="text-[10.5px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
              {t("risk.overallRisk")}
            </div>
            <div className="mt-1 flex min-w-0 items-center gap-2">
              <span className="text-[20px] font-semibold tracking-tight">
                {displayReview ? t(`risk.verdict.${displayReview.verdict}`) : t(`risk.level.${risk}`)}
              </span>
              {displayReview ? (
                <Pill tone={verdictTone[displayReview.verdict] ?? "default"}>
                  {displayReview.source === "remote" ? t("risk.remoteCached") : t("risk.localCached")}
                </Pill>
              ) : null}
            </div>
            <div className="mt-1 truncate text-[11.5px] font-mono text-[var(--fg-muted)]">
              {skillPath}
            </div>
          </div>
        </div>
      </Card>

      <Card className="p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-[13px] font-semibold">
              <Sparkles size={14} className="text-[var(--brand)]" />
              {t("risk.aiReview")}
            </div>
            <p className="mt-1 max-w-[560px] text-[12px] text-[var(--fg-muted)]">
              {aiConfigured ? t("risk.aiReviewDesc") : t("risk.aiNotConfigured")}
            </p>
          </div>
          <div className="flex flex-wrap justify-end gap-2">
            {needsRemoteSync ? (
              <Button
                size="sm"
                variant="outline"
                isDisabled={syncReview.isPending}
                onPress={() => syncReview.mutate(displayReview)}
              >
                {syncReview.isPending ? <Spinner size="sm" /> : <UploadCloud size={14} />}
                {syncReview.isPending ? t("risk.syncing") : t("risk.syncRemote")}
              </Button>
            ) : null}
            <Button
              size="sm"
              variant={aiConfigured ? "secondary" : "outline"}
              isDisabled={!aiConfigured || reviewBusy}
              onPress={handleRunReview}
            >
              {reviewBusy ? <Spinner size="sm" /> : <RefreshCw size={14} />}
              {refreshing
                ? t("risk.checkingLatest")
                : review.isPending
                  ? t("risk.aiReviewing")
                  : displayReview
                    ? t("risk.rerunReview")
                    : t("risk.aiRunReview")}
            </Button>
          </div>
        </div>

        {hashQuery.error ? (
          <ErrorBox error={hashQuery.error} />
        ) : review.error ? (
          <ErrorBox error={review.error} />
        ) : syncReview.error ? (
          <ErrorBox error={syncReview.error} />
        ) : null}

        {displayReview ? (
          <div className="mt-3 space-y-3 border-t border-[var(--line)] pt-3">
            {maybeOutdated ? (
              <div className="rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-[12px] text-[var(--warning)]">
                {t("risk.reviewStale")}
              </div>
            ) : null}
            <ReviewResultBlock review={displayReview} locale={locale} t={t} embedded />
          </div>
        ) : localReview.isPending ? null : (
          <div className="mt-3 rounded-md border border-dashed border-[var(--line)] px-3 py-4 text-[12px] text-[var(--fg-muted)]">
            {t("risk.noReview")}
          </div>
        )}
      </Card>
    </div>
  );
}

function ErrorBox({ error }: { error: unknown }) {
  return (
    <div className="mt-3 rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
      {formatError(error)}
    </div>
  );
}

function ReviewResultBlock({
  review,
  locale,
  t,
  embedded,
}: {
  review: DisplayReview;
  locale: string;
  t: (key: string) => string;
  embedded?: boolean;
}) {
  return (
    <div className={embedded ? "space-y-3" : "mt-3 space-y-3 border-t border-[var(--line)] pt-3"}>
      <div className="space-y-1.5">
        <div className="flex flex-wrap items-center gap-2">
          <Pill tone={verdictTone[review.verdict] ?? "default"}>
            {t(`risk.verdict.${review.verdict}`)}
          </Pill>
          {review.synced ? <Pill tone="success">{t("risk.synced")}</Pill> : null}
        </div>
        <div className="text-[12.5px] text-[var(--fg-secondary)]">{review.summary}</div>
        <div className="text-[11.5px] text-[var(--fg-muted)]">
          {formatAttribution(review, locale, t)}
        </div>
      </div>

      {review.findings.length ? (
        <div className="space-y-1.5">
          {review.findings.map((finding, i) => (
            <AiFindingRow key={`${finding.severity}:${i}`} finding={finding} />
          ))}
        </div>
      ) : (
        <div className="flex items-center gap-2 text-[12px] text-[var(--fg-muted)]">
          <CheckCircle2 size={14} className="text-[var(--success)]" />
          {t("risk.noFindings")}
        </div>
      )}
    </div>
  );
}

function AiFindingRow({ finding }: { finding: AiReviewFinding }) {
  const icon =
    finding.severity === "danger" ? (
      <AlertTriangle size={14} className="mt-0.5 shrink-0 text-[var(--danger)]" />
    ) : finding.severity === "warning" ? (
      <AlertTriangle size={14} className="mt-0.5 shrink-0 text-[var(--warning)]" />
    ) : (
      <Info size={14} className="mt-0.5 shrink-0 text-[var(--fg-muted)]" />
    );

  return (
    <div className="flex items-start gap-2 rounded-md border border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2 text-[12px]">
      {icon}
      <div className="min-w-0 flex-1 text-[var(--fg-secondary)]">{finding.detail}</div>
      <Pill tone={aiSeverityTone[finding.severity] ?? "default"}>{finding.severity}</Pill>
    </div>
  );
}

function formatAttribution(review: ReviewCacheEntry, locale: string, t: (key: string) => string): string {
  const actor = review.reviewedBy || t("risk.unknownReviewer");
  const when = formatRelativeTime(review.reviewedAt, locale);
  return [t("risk.reviewedBy").replace("{actor}", actor).replace("{when}", when), review.model]
    .filter(Boolean)
    .join(" · ");
}
