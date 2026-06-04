import { AlertDialog, Button, Drawer, Spinner, toast } from "@heroui/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ExternalLink,
  GitMerge,
  GitPullRequestArrow,
  MessageSquare,
  RefreshCw,
  XCircle,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import {
  addWorkspacePullRequestComment,
  closeWorkspacePullRequest,
  type PullRequestQueryState,
  type WorkspacePullRequest,
  listWorkspacePullRequestFiles,
  listWorkspacePullRequests,
  mergeWorkspacePullRequest,
} from "../lib/skill-library";
import { formatError, formatRelativeTime, openExternalUrl } from "../utils/format";
import { Card } from "../widgets/Card";
import { MetricTile } from "../widgets/MetricTile";
import { Pill } from "../widgets/Pill";
import { InlineFileDiff } from "../widgets/PublishModal";
import { SegmentedTabs } from "../widgets/SegmentedTabs";

const EMPTY_PRS: WorkspacePullRequest[] = [];

export function PublishPage({
  workspaceRef,
  providerName = "GitHub",
  supportsPullRequests = true,
  supportsPullRequestActions = true,
  pullRequestActionBlockedReason = null,
}: {
  workspaceRef: string;
  providerName?: string;
  supportsPullRequests?: boolean;
  supportsPullRequestActions?: boolean;
  pullRequestActionBlockedReason?: string | null;
}) {
  const { t } = useLocale();
  const queryClient = useQueryClient();
  const [state, setState] = useState<PullRequestQueryState>("open");
  const [selectedNumber, setSelectedNumber] = useState<number | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);

  const stateOptions: Array<{ id: PullRequestQueryState; label: string }> = [
    { id: "open", label: t("publishPage.stateOpen") },
    { id: "closed", label: t("publishPage.stateClosed") },
    { id: "all", label: t("publishPage.stateAll") },
  ];

  const query = useQuery({
    queryKey: ["pull-requests", workspaceRef, state],
    queryFn: () => listWorkspacePullRequests(workspaceRef, state),
    enabled: Boolean(workspaceRef && supportsPullRequests),
    staleTime: 60 * 1000,
  });

  const prs = query.data ?? EMPTY_PRS;
  const selected = useMemo(
    () => prs.find((pr) => pr.number === selectedNumber) ?? null,
    [prs, selectedNumber],
  );

  useEffect(() => {
    if (selected || selectedNumber === null) return;
    setSelectedNumber(null);
    setDetailOpen(false);
  }, [prs, selected, selectedNumber]);

  const closeDetail = (open: boolean) => {
    setDetailOpen(open);
    if (!open) setSelectedNumber(null);
  };

  const open = prs.filter((pr) => pr.state === "open" && !pr.merged).length;
  const merged = prs.filter((pr) => pr.merged).length;
  const drafts = prs.filter((pr) => pr.draft).length;

  const refreshPrs = () => {
    void query.refetch();
    void queryClient.invalidateQueries({ queryKey: ["pull-request-files", workspaceRef] });
  };

  const mergeMutation = useMutation({
    mutationFn: (pr: WorkspacePullRequest) =>
      mergeWorkspacePullRequest({
        workspace: workspaceRef,
        number: pr.number,
        headRef: pr.head_ref,
        headRepo: pr.head_repo,
        deleteBranch: true,
    }),
    onSuccess: (result) => {
      if (result.error) toast.warning(result.error);
      else toast.success(result.deletedBranch ? t("publishPage.toastMergedDeletedBranch") : t("publishPage.toastMerged"));
      refreshPrs();
    },
    onError: (err) => toast.danger(formatError(err)),
  });

  const closeMutation = useMutation({
    mutationFn: (input: { pr: WorkspacePullRequest; comment: string }) =>
      closeWorkspacePullRequest({
        workspace: workspaceRef,
        number: input.pr.number,
        comment: input.comment.trim() || null,
    }),
    onSuccess: () => {
      toast.success(t("publishPage.toastClosed"));
      refreshPrs();
    },
    onError: (err) => toast.danger(formatError(err)),
  });

  const commentMutation = useMutation({
    mutationFn: (input: { pr: WorkspacePullRequest; body: string }) =>
      addWorkspacePullRequestComment({
        workspace: workspaceRef,
        number: input.pr.number,
        body: input.body,
    }),
    onSuccess: () => {
      toast.success(t("publishPage.toastCommented"));
    },
    onError: (err) => toast.danger(formatError(err)),
  });

  if (!workspaceRef) {
    return (
      <section className="grid min-h-0 flex-1 place-items-center overflow-hidden px-6 py-6">
        <div className="empty-state mx-auto max-w-md">
          <div className="empty-state__title">{t("publishPage.pickWorkspace")}</div>
          <div>{t("publishPage.selectWorkspace")}</div>
        </div>
      </section>
    );
  }

  if (!supportsPullRequests) {
    return (
      <section className="grid min-h-0 flex-1 place-items-center overflow-hidden px-6 py-6">
        <div className="empty-state mx-auto max-w-md">
          <div className="empty-state__title">{t("publishPage.unsupportedTitle")}</div>
          <div>{t("publishPage.unsupportedDesc").replace("{provider}", providerName)}</div>
        </div>
      </section>
    );
  }

  return (
    <section className="flex min-h-0 flex-1 overflow-hidden px-6 py-6">
      <div className="mx-auto flex h-full min-h-0 w-full max-w-[1480px] flex-col gap-5">
        <div className="grid gap-3 md:grid-cols-3">
          <MetricTile label={t("publishPage.open")} value={open} tone={open ? "warning" : "default"} />
          <MetricTile label={t("publishPage.merged")} value={merged} tone={merged ? "success" : "default"} />
          <MetricTile label={t("publishPage.drafts")} value={drafts} tone={drafts ? "default" : "default"} />
        </div>

        <Card className="flex min-h-0 flex-1 flex-col overflow-hidden p-0 gap-0">
          <Card.Header>
            <div>
              <Card.Title>{t("publishPage.pullRequests")}</Card.Title>
              <Card.Subtitle className="truncate font-mono">{workspaceRef}</Card.Subtitle>
            </div>
            <div className="flex items-center gap-2">
              <SegmentedTabs<PullRequestQueryState>
                tabs={stateOptions.map((o) => ({ id: o.id, label: o.label }))}
                active={state}
                onChange={(next) => {
                  setState(next);
                  setSelectedNumber(null);
                  setDetailOpen(false);
                }}
              />
              <Button
                isIconOnly
                size="sm"
                variant="tertiary"
                onPress={() => query.refetch()}
                isPending={query.isFetching}
                aria-label={t("publishPage.refreshPullRequests")}
              >
                <RefreshCw size={13} />
              </Button>
            </div>
          </Card.Header>

          {query.error ? (
            <div className="border-b border-[var(--line)] bg-[var(--danger-soft)] px-4 py-2 text-[12px] text-[var(--danger)]">
              {formatError(query.error)}
            </div>
          ) : null}

          <div className="min-h-0 flex-1 overflow-y-auto">
            {prs.length ? (
              <div className="divide-y divide-[var(--line)]">
                {prs.map((pr) => (
                  <PullRequestRow
                    key={pr.number}
                    pr={pr}
                    selected={detailOpen && selected?.number === pr.number}
                    onSelect={() => {
                      setSelectedNumber(pr.number);
                      setDetailOpen(true);
                    }}
                  />
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
        </Card>

        <Drawer.Backdrop isOpen={detailOpen && Boolean(selected)} onOpenChange={closeDetail} variant="blur">
          <Drawer.Content placement="right" className="publish-pr-drawer__content">
            <Drawer.Dialog className="publish-pr-drawer" aria-label={selected?.title ?? t("publishPage.selectPr")}>
              <Drawer.CloseTrigger />
              {selected ? (
                <PullRequestDetail
                  workspaceRef={workspaceRef}
                  pr={selected}
                  mergePending={mergeMutation.isPending}
                  closePending={closeMutation.isPending}
                  commentPending={commentMutation.isPending}
                  supportsActions={supportsPullRequestActions}
                  actionBlockedReason={pullRequestActionBlockedReason}
                  onMerge={(pr) => mergeMutation.mutate(pr)}
                  onClose={(input) => closeMutation.mutate(input)}
                  onComment={(input) => commentMutation.mutate(input)}
                />
              ) : null}
            </Drawer.Dialog>
          </Drawer.Content>
        </Drawer.Backdrop>
      </div>
    </section>
  );
}

function PullRequestRow({
  pr,
  selected,
  onSelect,
}: {
  pr: WorkspacePullRequest;
  selected: boolean;
  onSelect: () => void;
}) {
  const { t } = useLocale();
  const tone: "success" | "warning" | "default" | "danger" = pr.merged
    ? "success"
    : pr.state === "open"
      ? "warning"
      : "default";
  const label = pr.merged ? t("publishPage.merged") : pr.state;

  return (
    <button
      type="button"
      onClick={onSelect}
      className={`card-row w-full text-left ${
        selected ? "bg-[var(--bg-soft)] shadow-[inset_3px_0_0_var(--brand)]" : ""
      }`}
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
          <span>{pr.author ? `@${pr.author}` : "-"}</span>
          <span>·</span>
          <span className="truncate font-mono">
            {pr.head_ref} → {pr.base_ref}
          </span>
          <span>·</span>
          <span>{formatRelativeTime(pr.updated_at)}</span>
        </div>
      </div>
      <div className="flex items-center gap-1.5">
        {pr.draft ? <Pill>{t("publishPage.statusDraft")}</Pill> : null}
        <Pill tone={tone}>{label}</Pill>
      </div>
    </button>
  );
}

function PullRequestDetail({
  workspaceRef,
  pr,
  mergePending,
  closePending,
  commentPending,
  supportsActions,
  actionBlockedReason,
  onMerge,
  onClose,
  onComment,
}: {
  workspaceRef: string;
  pr: WorkspacePullRequest | null;
  mergePending: boolean;
  closePending: boolean;
  commentPending: boolean;
  supportsActions: boolean;
  actionBlockedReason?: string | null;
  onMerge: (pr: WorkspacePullRequest) => void;
  onClose: (input: { pr: WorkspacePullRequest; comment: string }) => void;
  onComment: (input: { pr: WorkspacePullRequest; body: string }) => void;
}) {
  const { t } = useLocale();
  const [comment, setComment] = useState("");
  const [closeComment, setCloseComment] = useState("");
  const filesQuery = useQuery({
    queryKey: ["pull-request-files", workspaceRef, pr?.number],
    queryFn: () => listWorkspacePullRequestFiles({ workspace: workspaceRef, number: pr!.number }),
    enabled: Boolean(workspaceRef && pr),
    staleTime: 60 * 1000,
  });

  useEffect(() => {
    setComment("");
    setCloseComment("");
  }, [pr?.number]);

  if (!pr) {
    return (
      <div className="empty-state">
        <div className="empty-state__title">{t("publishPage.selectPr")}</div>
        <div>{t("publishPage.selectPr.desc")}</div>
      </div>
    );
  }

  const openPr = pr.state === "open" && !pr.merged;
  const files = filesQuery.data ?? [];
  const body = pr.body?.trim();
  const showBlockedToast = () => {
    if (actionBlockedReason) toast.warning(actionBlockedReason);
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-b border-[var(--line)] px-5 py-4 pr-14">
        <div className="flex items-start justify-between gap-5">
          <div className="min-w-0">
            <div className="flex min-w-0 items-center gap-2">
              {pr.merged ? (
                <GitMerge size={16} className="text-[var(--success)]" />
              ) : (
                <GitPullRequestArrow
                  size={16}
                  className={openPr ? "text-[var(--warning)]" : "text-[var(--fg-muted)]"}
                />
              )}
              <h2 className="truncate text-[15px] font-semibold tracking-tight">{pr.title}</h2>
              <span className="font-mono text-[12px] text-[var(--fg-muted)]">#{pr.number}</span>
            </div>
            <div className="mt-1 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-[11.5px] text-[var(--fg-muted)]">
              <span>{pr.author ? `@${pr.author}` : "-"}</span>
              <span>·</span>
              <span className="font-mono">{pr.head_ref}</span>
              <span>→</span>
              <span className="font-mono">{pr.base_ref}</span>
              <span>·</span>
              <span>{formatRelativeTime(pr.updated_at)}</span>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2 pr-1">
            <Button
              isIconOnly
              size="sm"
              variant="tertiary"
              onPress={() => filesQuery.refetch()}
              isPending={filesQuery.isFetching}
              aria-label={t("publishPage.refreshDiff")}
            >
              <RefreshCw size={13} />
            </Button>
            <Button
              isIconOnly
              size="sm"
              variant="tertiary"
              onPress={() => void openExternalUrl(pr.html_url)}
              aria-label={t("publishPage.openOnGithub")}
            >
              <ExternalLink size={13} />
            </Button>
          </div>
        </div>

        {body ? (
          <div className="mt-3 max-h-28 overflow-auto rounded-md border border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2 text-[12px] leading-5 text-[var(--fg-secondary)]">
            {body}
          </div>
        ) : null}

        {openPr && supportsActions ? (
          <div className="mt-3 flex flex-wrap items-center gap-2">
            {actionBlockedReason ? (
              <Button size="sm" variant="outline" className="opacity-60" onPress={showBlockedToast}>
                <GitMerge size={14} />
                {t("publishPage.merge")}
              </Button>
            ) : (
              <AlertDialog>
                <Button size="sm" variant="outline" isDisabled={mergePending || closePending}>
                  <GitMerge size={14} />
                  {t("publishPage.merge")}
                </Button>
                <AlertDialog.Backdrop>
                  <AlertDialog.Container size="sm">
                    <AlertDialog.Dialog className="sm:max-w-[420px]">
                      <AlertDialog.CloseTrigger />
                      <AlertDialog.Header>
                        <AlertDialog.Icon status="success" />
                        <AlertDialog.Heading>{t("publishPage.mergeTitle")}</AlertDialog.Heading>
                      </AlertDialog.Header>
                      <AlertDialog.Body>
                        <div className="space-y-2 text-[13px] leading-[1.5] text-[var(--fg-secondary)]">
                          <p>{t("publishPage.mergeDesc")}</p>
                          <div className="rounded-md border border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2">
                            <div className="truncate font-medium text-[var(--fg)]">{pr.title}</div>
                            <div className="mt-0.5 truncate font-mono text-[11px] text-[var(--fg-muted)]">
                              {pr.head_ref} → {pr.base_ref}
                            </div>
                          </div>
                        </div>
                      </AlertDialog.Body>
                      <AlertDialog.Footer>
                        <Button slot="close" variant="outline">
                          {t("common.cancel")}
                        </Button>
                        <Button
                          slot="close"
                          onPress={() => onMerge(pr)}
                          isPending={mergePending}
                        >
                          {t("publishPage.confirmMerge")}
                        </Button>
                      </AlertDialog.Footer>
                    </AlertDialog.Dialog>
                  </AlertDialog.Container>
                </AlertDialog.Backdrop>
              </AlertDialog>
            )}

            {actionBlockedReason ? (
              <Button size="sm" variant="danger-soft" className="opacity-60" onPress={showBlockedToast}>
                <XCircle size={14} />
                {t("publishPage.rejectClose")}
              </Button>
            ) : (
              <AlertDialog>
                <Button size="sm" variant="danger-soft" isDisabled={mergePending || closePending}>
                  <XCircle size={14} />
                  {t("publishPage.rejectClose")}
                </Button>
                <AlertDialog.Backdrop>
                  <AlertDialog.Container size="sm">
                    <AlertDialog.Dialog className="sm:max-w-[460px]">
                      <AlertDialog.CloseTrigger />
                      <AlertDialog.Header>
                        <AlertDialog.Icon status="danger" />
                        <AlertDialog.Heading>{t("publishPage.closeTitle")}</AlertDialog.Heading>
                      </AlertDialog.Header>
                      <AlertDialog.Body>
                        <div className="space-y-3 text-[13px] leading-[1.5] text-[var(--fg-secondary)]">
                          <p>{t("publishPage.closeDesc")}</p>
                          <textarea
                            value={closeComment}
                            onChange={(event) => setCloseComment(event.target.value)}
                            rows={5}
                            className="w-full resize-y rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2 text-[13px] text-[var(--fg)] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                            placeholder={t("publishPage.closePlaceholder")}
                          />
                        </div>
                      </AlertDialog.Body>
                      <AlertDialog.Footer>
                        <Button slot="close" variant="outline">
                          {t("common.cancel")}
                        </Button>
                        <Button
                          slot="close"
                          variant="danger-soft"
                          onPress={() => onClose({ pr, comment: closeComment })}
                          isPending={closePending}
                        >
                          {t("publishPage.confirmClose")}
                        </Button>
                      </AlertDialog.Footer>
                    </AlertDialog.Dialog>
                  </AlertDialog.Container>
                </AlertDialog.Backdrop>
              </AlertDialog>
            )}
          </div>
        ) : null}
      </div>

      <div className={supportsActions ? "grid min-h-0 flex-1 grid-cols-1 xl:grid-cols-[minmax(0,1fr)_340px]" : "min-h-0 flex-1"}>
        <div className="min-h-0 overflow-y-auto bg-[var(--bg-soft)] px-4 py-3">
          {filesQuery.isFetching && !files.length ? (
            <div className="flex items-center justify-center gap-2 rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-4 py-10 text-[12px] text-[var(--fg-muted)]">
              <Spinner size="sm" />
              {t("publishPage.loadingDiff")}
            </div>
          ) : filesQuery.error ? (
            <div className="rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
              {formatError(filesQuery.error)}
            </div>
          ) : files.length ? (
            <div className="space-y-4">
              {files.map((file) => (
                <InlineFileDiff key={file.filename} file={file} />
              ))}
            </div>
          ) : (
            <div className="rounded-md border border-dashed border-[var(--line)] bg-[var(--bg-elevated)] px-4 py-10 text-center text-[12px] text-[var(--fg-muted)]">
              {t("publishPage.noFileChanges")}
            </div>
          )}
        </div>

        {supportsActions ? (
          <aside className="flex min-h-0 flex-col border-t border-[var(--line)] bg-[var(--bg-elevated)] xl:border-l xl:border-t-0">
            <div className="border-b border-[var(--line)] px-4 py-3">
              <div className="text-[12px] font-semibold text-[var(--fg)]">{t("publishPage.comments")}</div>
              <div className="mt-0.5 text-[11.5px] text-[var(--fg-muted)]">{t("publishPage.commentsDesc")}</div>
            </div>
            <div className="flex min-h-0 flex-1 flex-col gap-3 px-4 py-3">
              <textarea
                value={comment}
                onChange={(event) => setComment(event.target.value)}
                rows={8}
                className="w-full resize-y rounded-md border border-[var(--line)] bg-[var(--bg)] px-3 py-2 text-[13px] text-[var(--fg)] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                placeholder={t("publishPage.commentPlaceholder")}
              />
              <Button
                size="sm"
                onPress={() => {
                  if (actionBlockedReason) {
                    showBlockedToast();
                    return;
                  }
                  onComment({ pr, body: comment });
                  setComment("");
                }}
                isDisabled={!comment.trim() || commentPending}
                isPending={commentPending}
              >
                <MessageSquare size={14} />
                {t("publishPage.submitComment")}
              </Button>
              <div className={`mt-auto rounded-md border px-3 py-2 text-[11.5px] leading-5 ${
                actionBlockedReason
                  ? "border-[var(--warning)] bg-[var(--warning-soft)] text-[var(--warning)]"
                  : "border-[var(--line)] bg-[var(--bg-soft)] text-[var(--fg-muted)]"
              }`}>
                {actionBlockedReason ?? t("publishPage.commentNote")}
              </div>
            </div>
          </aside>
        ) : null}
      </div>
    </div>
  );
}

// keep these re-exports for backward-compat with old App.tsx references
export type { WorkspacePullRequest as PublishRequestRecord, WorkspacePullRequest as PublishPolicyCheckRecord } from "../lib/skill-library";
