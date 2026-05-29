import { Button } from "@heroui/react";
import { useQuery } from "@tanstack/react-query";
import { ExternalLink, GitMerge, GitPullRequestArrow, RefreshCw } from "lucide-react";
import { useState } from "react";
import { useLocale } from "../hooks/useLocale";
import {
  type PullRequestQueryState,
  type WorkspacePullRequest,
  listWorkspacePullRequests,
} from "../lib/teamai";
import { formatRelativeTime } from "../utils/format";
import { openExternalUrl } from "../utils/format";
import { MetricTile } from "../widgets/MetricTile";
import { Pill } from "../widgets/Pill";
import { SegmentedTabs } from "../widgets/SegmentedTabs";

export function PublishPage({ workspaceRef }: { workspaceRef: string }) {
  const { t } = useLocale();
  const [state, setState] = useState<PullRequestQueryState>("open");

  const stateOptions: Array<{ id: PullRequestQueryState; label: string }> = [
    { id: "open", label: t("publishPage.stateOpen") },
    { id: "closed", label: t("publishPage.stateClosed") },
    { id: "all", label: t("publishPage.stateAll") },
  ];

  const query = useQuery({
    queryKey: ["pull-requests", workspaceRef, state],
    queryFn: () => listWorkspacePullRequests(workspaceRef, state),
    enabled: Boolean(workspaceRef),
    staleTime: 60 * 1000,
  });

  const prs = query.data ?? [];
  const open = prs.filter((pr) => pr.state === "open" && !pr.merged).length;
  const merged = prs.filter((pr) => pr.merged).length;
  const drafts = prs.filter((pr) => pr.draft).length;

  if (!workspaceRef) {
    return (
      <section className="scroll-area min-h-0 flex-1 px-6 py-6">
        <div className="empty-state mx-auto max-w-md">
          <div className="empty-state__title">{t("publishPage.pickWorkspace")}</div>
          <div>{t("publishPage.selectWorkspace")}</div>
        </div>
      </section>
    );
  }

  return (
    <section className="scroll-area min-h-0 flex-1 px-6 py-6">
      <div className="mx-auto flex max-w-5xl flex-col gap-5">
        <div className="grid gap-3 md:grid-cols-3">
          <MetricTile label={t("publishPage.open")} value={open} tone={open ? "warning" : "default"} />
          <MetricTile label={t("publishPage.merged")} value={merged} tone={merged ? "success" : "default"} />
          <MetricTile label={t("publishPage.drafts")} value={drafts} tone={drafts ? "default" : "default"} />
        </div>

        <div className="card overflow-hidden">
          <div className="card-header">
            <div>
              <div className="card-title">{t("publishPage.pullRequests")}</div>
              <div className="card-subtitle truncate font-mono">{workspaceRef}</div>
            </div>
            <div className="flex items-center gap-2">
              <SegmentedTabs<PullRequestQueryState>
                tabs={stateOptions.map((o) => ({ id: o.id, label: o.label }))}
                active={state}
                onChange={setState}
              />
              <Button
                isIconOnly
                size="sm"
                variant="tertiary"
                onPress={() => query.refetch()}
                isPending={query.isFetching}
              >
                <RefreshCw size={13} />
              </Button>
            </div>
          </div>

          {query.error ? (
            <div className="border-b border-[var(--line)] bg-[var(--danger-soft)] px-4 py-2 text-[12px] text-[var(--danger)]">
              {query.error instanceof Error ? query.error.message : String(query.error)}
            </div>
          ) : null}

          {prs.length ? (
            <div className="divide-y divide-[var(--line)]">
              {prs.map((pr) => (
                <PullRequestRow key={pr.number} pr={pr} />
              ))}
            </div>
          ) : query.isFetching ? (
            <div className="empty-state">
              <div className="empty-state__title">{t("publishPage.loading")}</div>
              <div>{t("publishPage.fetchingPrs")}</div>
            </div>
          ) : (
            <div className="empty-state">
              <div className="empty-state__title">{t("publishPage.noPrs").replace("{state}", state)}</div>
              <div>{t("publishPage.useSyncHint")}</div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function PullRequestRow({ pr }: { pr: WorkspacePullRequest }) {
  const tone: "success" | "warning" | "default" | "danger" = pr.merged
    ? "success"
    : pr.state === "open"
      ? "warning"
      : "default";
  const label = pr.merged ? "merged" : pr.state;
  return (
    <button
      type="button"
      onClick={() => void openExternalUrl(pr.html_url)}
      className="card-row w-full text-left"
    >
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          {pr.merged ? (
            <GitMerge size={13} className="text-[var(--success)]" />
          ) : (
            <GitPullRequestArrow
              size={13}
              className={pr.state === "open" ? "text-[var(--warning)]" : "text-[var(--fg-muted)]"}
            />
          )}
          <span className="truncate text-[13px] font-medium">{pr.title}</span>
          <span className="font-mono text-[11.5px] text-[var(--fg-muted)]">#{pr.number}</span>
        </div>
        <div className="mt-1 flex items-center gap-1.5 truncate text-[11.5px] text-[var(--fg-muted)]">
          <span>{pr.author ? `@${pr.author}` : "—"}</span>
          <span>·</span>
          <span className="truncate font-mono">
            {pr.head_ref} → {pr.base_ref}
          </span>
          <span>·</span>
          <span>{formatRelativeTime(pr.updated_at)}</span>
        </div>
      </div>
      <div className="flex items-center gap-1.5">
        {pr.draft ? <Pill>draft</Pill> : null}
        <Pill tone={tone}>{label}</Pill>
        <ExternalLink size={12} className="text-[var(--fg-muted)]" />
      </div>
    </button>
  );
}

// keep these re-exports for backward-compat with old App.tsx references
export type { WorkspacePullRequest as PublishRequestRecord, WorkspacePullRequest as PublishPolicyCheckRecord } from "../lib/teamai";
