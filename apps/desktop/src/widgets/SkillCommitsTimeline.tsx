import { Button } from "@heroui/react";
import { useQuery } from "@tanstack/react-query";
import { ExternalLink, GitCommit, RefreshCw } from "lucide-react";
import { listSkillCommits, type SkillCommit } from "../lib/teamai";
import { useLocale } from "../hooks/useLocale";
import { formatRelativeTime, openExternalUrl } from "../utils/format";

export function SkillCommitsTimeline({
  workspace,
  skillPath,
  refName,
}: {
  workspace: string;
  skillPath: string;
  refName?: string;
}) {
  const { t } = useLocale();
  const query = useQuery({
    queryKey: ["skill-commits", workspace, skillPath, refName ?? null],
    queryFn: () => listSkillCommits({ workspace, skillPath, refName, limit: 50 }),
    enabled: Boolean(workspace && skillPath),
    staleTime: 60 * 1000,
  });

  const commits = query.data ?? [];

  if (!workspace || !skillPath) {
    return (
      <div className="empty-state">
        <div className="empty-state__title">{t("commits.pickSkill")}</div>
        <div>{t("commits.selectSkill")}</div>
      </div>
    );
  }

  return (
    <div>
      <div className="mb-3 flex items-center justify-between">
        <div className="text-[11.5px] text-[var(--fg-muted)]">
          {commits.length} {commits.length === 1 ? t("commits.commit") : t("commits.commits")} {t("commits.touching")}{" "}
          <span className="font-mono">{skillPath}</span>
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
        <div className="rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
          {query.error instanceof Error ? query.error.message : String(query.error)}
        </div>
      ) : commits.length ? (
        <div className="commit-list">
          {commits.map((commit) => (
            <CommitRow key={commit.sha} commit={commit} />
          ))}
        </div>
      ) : query.isFetching ? (
        <div className="empty-state">
          <div className="empty-state__title">{t("commits.loading")}</div>
        </div>
      ) : (
        <div className="empty-state">
          <GitCommit size={20} className="text-[var(--fg-muted)]" />
          <div className="empty-state__title">{t("commits.noCommits")}</div>
          <div>{t("commits.noCommitsDesc")}</div>
        </div>
      )}
    </div>
  );
}

function CommitRow({ commit }: { commit: SkillCommit }) {
  return (
    <button
      type="button"
      className="commit-row"
      onClick={() => void openExternalUrl(commit.html_url)}
    >
      <div className="commit-row__bullet">
        <GitCommit size={11} />
      </div>
      <div className="min-w-0">
        <div className="commit-row__msg">{commit.message}</div>
        <div className="commit-row__meta">
          <span className="font-mono">{commit.short_sha}</span>
          <span>·</span>
          <span>{commit.author ? `@${commit.author}` : commit.author_email ?? "—"}</span>
          <span>·</span>
          <span>{formatRelativeTime(commit.authored_at)}</span>
        </div>
      </div>
      <ExternalLink size={12} className="text-[var(--fg-muted)]" />
    </button>
  );
}
