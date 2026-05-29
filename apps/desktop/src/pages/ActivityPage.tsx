import { Button } from "@heroui/react";
import { useQuery } from "@tanstack/react-query";
import {
  Activity as ActivityIcon,
  ExternalLink,
  GitBranch,
  GitCommit,
  GitMerge,
  GitPullRequestArrow,
  RefreshCw,
  Tag,
} from "lucide-react";
import type { ReactNode } from "react";
import { useLocale } from "../hooks/useLocale";
import { listWorkspaceEvents, type WorkspaceEvent } from "../lib/teamai";
import { formatRelativeTime, openExternalUrl } from "../utils/format";
import { MetricTile } from "../widgets/MetricTile";

const eventIcon: Record<string, ReactNode> = {
  PushEvent: <GitCommit size={13} className="text-[var(--brand)]" />,
  PullRequestEvent: <GitPullRequestArrow size={13} className="text-[var(--warning)]" />,
  ReleaseEvent: <Tag size={13} className="text-[var(--success)]" />,
  CreateEvent: <GitBranch size={13} className="text-[var(--fg-muted)]" />,
  MergeEvent: <GitMerge size={13} className="text-[var(--success)]" />,
};

export function ActivityPage({ workspaceRef }: { workspaceRef: string }) {
  const { t } = useLocale();
  const query = useQuery({
    queryKey: ["workspace-events", workspaceRef],
    queryFn: () => listWorkspaceEvents(workspaceRef),
    enabled: Boolean(workspaceRef),
    staleTime: 60 * 1000,
  });

  const events = query.data ?? [];
  const pushCount = events.filter((e) => e.event_type === "PushEvent").length;
  const prCount = events.filter((e) => e.event_type === "PullRequestEvent").length;
  const releaseCount = events.filter((e) => e.event_type === "ReleaseEvent").length;

  if (!workspaceRef) {
    return (
      <section className="scroll-area min-h-0 flex-1 px-6 py-6">
        <div className="empty-state mx-auto max-w-md">
          <div className="empty-state__title">{t("activity.pickWorkspace")}</div>
          <div>{t("activity.selectWorkspace")}</div>
        </div>
      </section>
    );
  }

  return (
    <section className="scroll-area min-h-0 flex-1 px-6 py-6">
      <div className="mx-auto flex max-w-5xl flex-col gap-5">
        <div className="grid gap-3 md:grid-cols-3">
          <MetricTile label={t("activity.pushes")} value={pushCount} tone={pushCount ? "warning" : "default"} />
          <MetricTile label={t("activity.prEvents")} value={prCount} tone={prCount ? "default" : "default"} />
          <MetricTile label={t("activity.releases")} value={releaseCount} tone={releaseCount ? "success" : "default"} />
        </div>

        <div className="card overflow-hidden">
          <div className="card-header">
            <div>
              <div className="card-title">{t("activity.recentActivity")}</div>
              <div className="card-subtitle truncate font-mono">{workspaceRef}</div>
            </div>
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

          {query.error ? (
            <div className="border-b border-[var(--line)] bg-[var(--danger-soft)] px-4 py-2 text-[12px] text-[var(--danger)]">
              {query.error instanceof Error ? query.error.message : String(query.error)}
            </div>
          ) : null}

          {events.length ? (
            <div className="divide-y divide-[var(--line)]">
              {events.map((event) => (
                <EventRow key={event.id} event={event} />
              ))}
            </div>
          ) : query.isFetching ? (
            <div className="empty-state">
              <div className="empty-state__title">{t("activity.loading")}</div>
              <div>{t("activity.fetchingEvents")}</div>
            </div>
          ) : (
            <div className="empty-state">
              <ActivityIcon size={20} className="text-[var(--fg-muted)]" />
              <div className="empty-state__title">{t("activity.noActivity")}</div>
              <div>{t("activity.noActivityDesc")}</div>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function EventRow({ event }: { event: WorkspaceEvent }) {
  const icon = eventIcon[event.event_type] ?? <ActivityIcon size={13} className="text-[var(--fg-muted)]" />;
  const Wrapper: "button" | "div" = event.html_url ? "button" : "div";
  const wrapperProps = event.html_url
    ? {
        type: "button" as const,
        onClick: () => void openExternalUrl(event.html_url!),
        className: "card-row w-full text-left",
      }
    : { className: "card-row" };

  return (
    <Wrapper {...wrapperProps}>
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          {icon}
          <span className="truncate text-[13px] font-medium">{event.summary}</span>
        </div>
        <div className="mt-1 flex items-center gap-1.5 text-[11.5px] text-[var(--fg-muted)]">
          <span>{event.actor ? `@${event.actor}` : "—"}</span>
          <span>·</span>
          <span>{formatRelativeTime(event.created_at)}</span>
        </div>
      </div>
      {event.html_url ? <ExternalLink size={12} className="text-[var(--fg-muted)]" /> : null}
    </Wrapper>
  );
}
