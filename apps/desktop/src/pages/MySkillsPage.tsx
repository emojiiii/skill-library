import { Button, Modal, ProgressBar, Spinner, Switch, Tooltip, toast } from "@heroui/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, ChevronDown, Download, GitPullRequestArrow, Package, PackageOpen, RefreshCw, RotateCcw, ShieldAlert, ShieldCheck, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
  type AiReviewResult,
  dbDisableSkill,
  dbEnableSkill,
  dbImportSkill,
  dbListRuntimes,
  dbListSkills,
  dbScanUnmanaged,
  dbUnmanageSkill,
  downloadSkillAsync,
  type ManagedSkill,
  onSkillDownloadProgress,
  reviewLocalSkill,
  syncNow,
  type UnmanagedSkillInfo,
} from "../lib/teamai";
import { useLocale } from "../hooks/useLocale";
import { useLocalStorage } from "../hooks/useLocalStorage";
import { useTheme } from "../hooks/useTheme";
import { useAppStore } from "../state/appStore";
import { Card } from "../widgets/Card";
import { Pill, type PillTone } from "../widgets/Pill";

const TOOL_LABELS: Record<string, string> = {
  "claude-code": "Claude Code",
  cursor: "Cursor",
  codex: "Codex",
};

/** AI review verdict → Pill tone. */
const VERDICT_TONE: Record<string, PillTone> = {
  safe: "success",
  caution: "warning",
  danger: "danger",
};

/** AI finding severity → Pill tone. */
const SEVERITY_TONE: Record<string, PillTone> = {
  info: "default",
  warning: "warning",
  danger: "danger",
};

/**
 * "My skills" — the single home for skills installed on this machine. Merges
 * what used to be two screens (consumer "My skills" + power-user "Local"):
 *   - stat cards (managed / active integrations / runtimes detected)
 *   - import from IDE (scan + adopt unmanaged skills)
 *   - per-skill, per-runtime enable toggles
 *   - auto-update toggle + check-for-updates
 *   - push a local skill to a team workspace
 *   - remove
 * All backed by the same SQLite source (dbListSkills).
 */
export function MySkillsPage() {
  const { t } = useLocale();
  const queryClient = useQueryClient();
  const [showImport, setShowImport] = useState(false);

  // Push-to-workspace uses the global PushModal (lives in RootLayout).
  const setPushEntry = useAppStore((s) => s.setPushEntry);
  const setPushPreview = useAppStore((s) => s.setPushPreview);
  const setPushOpen = useAppStore((s) => s.setPushOpen);

  // AI provider config (same Settings the discover-page review uses).
  const settings = useTheme();
  const aiConfigured = settings.aiProvider !== "none" && Boolean(settings.aiBaseUrl);

  const skills = useQuery({ queryKey: ["db-skills"], queryFn: dbListSkills, staleTime: 30 * 1000 });
  const runtimes = useQuery({ queryKey: ["db-runtimes"], queryFn: dbListRuntimes, staleTime: 5 * 60 * 1000 });
  const unmanaged = useQuery({
    queryKey: ["db-unmanaged"],
    queryFn: dbScanUnmanaged,
    staleTime: 30 * 1000,
    enabled: showImport,
  });

  // Live download progress: each event patches the cached skill list in place so
  // the bar advances smoothly without a full refetch; on terminal states we also
  // invalidate so links/targets and the final row settle.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void onSkillDownloadProgress((p) => {
      queryClient.setQueryData<ManagedSkill[]>(["db-skills"], (prev) =>
        prev?.map((s) =>
          s.id === p.skillId
            ? { ...s, installStatus: p.status, downloadProgress: p.progress, downloadError: p.error ?? "" }
            : s,
        ),
      );
      if (p.status === "installed" || p.status === "error") {
        queryClient.invalidateQueries({ queryKey: ["db-skills"] });
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [queryClient]);

  const enable = useMutation({
    mutationFn: (a: { skillId: string; runtime: string }) => dbEnableSkill(a.skillId, a.runtime),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
  });
  const disable = useMutation({
    mutationFn: (a: { skillId: string; runtime: string }) => dbDisableSkill(a.skillId, a.runtime),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
  });
  const remove = useMutation({
    mutationFn: (skillId: string) => dbUnmanageSkill(skillId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
  });
  const update = useMutation({
    mutationFn: () => syncNow(true),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
  });
  const importSkill = useMutation({
    mutationFn: (a: { skillId: string; sourcePath: string }) => dbImportSkill(a.skillId, a.sourcePath),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["db-skills"] });
      queryClient.invalidateQueries({ queryKey: ["db-unmanaged"] });
    },
  });
  // Retry a failed/interrupted download. The 'error' row carries the source
  // workspace + in-repo path it was first downloaded from, so we can re-trigger
  // the async download with the same enabled targets.
  const retry = useMutation({
    mutationFn: (skill: ManagedSkill) =>
      downloadSkillAsync({
        workspace: skill.sourceWorkspace,
        assetId: skill.id,
        skillPath: skill.sourcePath || undefined,
        version: skill.version || undefined,
        name: skill.name,
        description: skill.description,
        targets: skill.targets.filter((tg) => tg.enabled).map((tg) => tg.runtime),
        linkMode: skill.linkMode || undefined,
      }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
    onError: (err) => toast.danger(String((err as { message?: string })?.message ?? err)),
  });
  // AI safety review of an installed skill, straight from its local copy. The
  // verdict is cached server-side; we invalidate so the card picks it up.
  const review = useMutation<AiReviewResult, Error, string>({
    mutationFn: (skillId: string) =>
      reviewLocalSkill({
        skillId,
        provider: settings.aiProvider,
        baseUrl: settings.aiBaseUrl,
        model: settings.aiModel,
      }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
    onError: (err) => {
      const code = (err as { code?: string })?.code;
      if (code === "ai_not_configured" || code === "ai_missing_key") {
        toast.warning(t("mySkills.aiNotConfigured"));
      } else {
        toast.danger(String((err as { message?: string })?.message ?? err));
      }
    },
  });
  const list = skills.data ?? [];
  const tools = (runtimes.data ?? []).filter((r) => r.exists);
  const totalEnabled = list.reduce((sum, s) => sum + s.targets.filter((tg) => tg.enabled).length, 0);

  const busyRuntimeKey = enable.isPending
    ? `${enable.variables?.skillId}:${enable.variables?.runtime}`
    : disable.isPending
      ? `${disable.variables?.skillId}:${disable.variables?.runtime}`
      : null;

  // Group unmanaged skills by source runtime for the import modal.
  const groupedUnmanaged = useMemo(() => {
    const items = unmanaged.data ?? [];
    const groups = new Map<string, UnmanagedSkillInfo[]>();
    for (const skill of items) {
      for (const runtime of skill.foundIn) {
        const arr = groups.get(runtime) ?? [];
        arr.push(skill);
        groups.set(runtime, arr);
      }
    }
    return [...groups.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [unmanaged.data]);

  return (
    <section className="scroll-area min-h-0 flex-1">
      <div className="mx-auto max-w-4xl p-6">
        {/* Stats */}
        <div className="mb-5 grid grid-cols-3 gap-3">
          <Stat label={t("local.managed")} value={list.length} />
          <Stat label={t("local.activeIntegrations")} value={totalEnabled} />
          <Stat label={t("local.runtimesDetected")} value={tools.length} />
        </div>

        {/* Toolbar */}
        <div className="mb-4 flex items-center gap-2">
          <Button size="sm" variant={showImport ? "secondary" : "outline"} onPress={() => setShowImport(true)}>
            <Download size={13} />
            {t("local.importFromIde")}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onPress={() => update.mutate()}
            isPending={update.isPending}
            isDisabled={!list.length}
          >
            <RotateCcw size={13} />
            {t("mySkills.checkUpdates")}
          </Button>
          <Button size="sm" variant="outline" onPress={() => skills.refetch()} isPending={skills.isFetching}>
            <RefreshCw size={13} />
            {t("local.refresh")}
          </Button>
          <span className="ml-auto text-[12px] text-[var(--fg-muted)]">
            {list.length} {t("mySkills.count")}
          </span>
        </div>

        {/* Import modal */}
        <Modal isOpen={showImport} onOpenChange={setShowImport}>
          <Modal.Backdrop>
            <Modal.Container size="lg">
              <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
                <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
                  <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                    {t("local.unmanaged")}
                  </Modal.Heading>
                  <p className="mt-1 text-[12px] text-[var(--fg-muted)]">{t("local.unmanaged.desc")}</p>
                </Modal.Header>

                <Modal.Body className="max-h-[60vh] overflow-y-auto px-5 py-4">
                  {unmanaged.isFetching && !groupedUnmanaged.length ? (
                    <div className="flex items-center justify-center py-8">
                      <Pill>{t("local.scanning")}</Pill>
                    </div>
                  ) : groupedUnmanaged.length ? (
                    <div className="overflow-hidden rounded-md border border-[var(--line)]">
                      {groupedUnmanaged.map(([runtime, items]) => (
                        <div key={runtime}>
                          <div className="sticky top-0 z-10 flex items-center justify-between border-b border-[var(--line)] bg-[var(--bg-soft)] px-4 py-2">
                            <div className="flex items-center gap-2">
                              <span className="text-[11px] font-semibold uppercase tracking-wide text-[var(--fg-muted)]">
                                {runtime}
                              </span>
                              <span className="text-[11px] text-[var(--fg-muted)]">({items.length})</span>
                            </div>
                            <Button
                              size="sm"
                              variant="secondary"
                              onPress={() => {
                                for (const skill of items) {
                                  importSkill.mutate({ skillId: skill.id, sourcePath: skill.path });
                                }
                              }}
                            >
                              <Download size={12} />
                              {t("local.importAll")}
                            </Button>
                          </div>
                          {items.map((skill) => (
                            <div
                              key={`${runtime}-${skill.id}`}
                              className="flex items-center justify-between gap-3 border-b border-[var(--line)] px-4 py-3 last:border-b-0 hover:bg-[var(--bg-soft)]"
                            >
                              <div className="min-w-0 flex-1">
                                <div className="flex items-center gap-2">
                                  <Package size={14} className="shrink-0 text-[var(--fg-muted)]" />
                                  <span className="truncate text-[13px] font-medium">{skill.name}</span>
                                </div>
                                <div className="mt-0.5 truncate font-mono text-[11px] text-[var(--fg-muted)]">
                                  {skill.id}
                                </div>
                              </div>
                              <Button
                                size="sm"
                                variant="secondary"
                                onPress={() => importSkill.mutate({ skillId: skill.id, sourcePath: skill.path })}
                                isPending={importSkill.isPending && importSkill.variables?.skillId === skill.id}
                              >
                                <Download size={12} />
                                {t("local.import")}
                              </Button>
                            </div>
                          ))}
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="flex items-center justify-center py-8 text-[12px] text-[var(--fg-muted)]">
                      {t("local.noUnmanaged")}
                    </div>
                  )}
                </Modal.Body>

                <div className="flex justify-end border-t border-[var(--line)] px-5 py-3">
                  <Button size="sm" variant="outline" onPress={() => setShowImport(false)}>
                    {t("common.close")}
                  </Button>
                </div>
              </Modal.Dialog>
            </Modal.Container>
          </Modal.Backdrop>
        </Modal>

        {/* Skill list */}
        {list.length === 0 ? (
          <div className="empty-state rounded-md border border-dashed border-[var(--line)]">
            <PackageOpen size={22} className="text-[var(--fg-muted)]" />
            <div className="empty-state__title">{t("mySkills.empty")}</div>
            <div>{t("mySkills.empty.desc")}</div>
          </div>
        ) : (
          <div className="space-y-3">
            {list.map((skill) => (
              <SkillCard
                key={skill.id}
                skill={skill}
                tools={tools.map((r) => r.id)}
                busyRuntimeKey={busyRuntimeKey}
                onToggle={(runtime, enabled) =>
                  enabled
                    ? disable.mutate({ skillId: skill.id, runtime })
                    : enable.mutate({ skillId: skill.id, runtime })
                }
                onRemove={() => remove.mutate(skill.id)}
                removing={remove.isPending && remove.variables === skill.id}
                onRetry={() => retry.mutate(skill)}
                retrying={retry.isPending && retry.variables?.id === skill.id}
                onReview={() => review.mutate(skill.id)}
                reviewing={review.isPending && review.variables === skill.id}
                aiConfigured={aiConfigured}
                onPush={() => {
                  setPushEntry({
                    id: skill.id,
                    name: skill.name,
                    path: skill.localPath,
                    hasManifest: true,
                    hasSkillMd: true,
                    managed: true,
                    version: skill.version || null,
                    description: skill.description || null,
                  });
                  setPushPreview(null);
                  setPushOpen(true);
                }}
              />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <Card className="px-4 py-3">
      <div className="text-[10.5px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">{label}</div>
      <div className="mt-1 text-[24px] font-semibold tracking-tight tabular-nums">{value}</div>
    </Card>
  );
}

function SkillCard({
  skill,
  tools,
  busyRuntimeKey,
  onToggle,
  onRemove,
  removing,
  onRetry,
  retrying,
  onReview,
  reviewing,
  aiConfigured,
  onPush,
}: {
  skill: ManagedSkill;
  tools: string[];
  busyRuntimeKey: string | null;
  onToggle: (runtime: string, currentlyEnabled: boolean) => void;
  onRemove: () => void;
  removing: boolean;
  onRetry: () => void;
  retrying: boolean;
  onReview: () => void;
  reviewing: boolean;
  aiConfigured: boolean;
  onPush: () => void;
}) {
  const { t } = useLocale();
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [showFindings, setShowFindings] = useState(false);
  const [autoUpdate, setAutoUpdate] = useLocalStorage<boolean>(`my-skills:auto:${skill.id}`, true);

  const isDownloading = skill.installStatus === "downloading";
  const isError = skill.installStatus === "error";
  const hasReview = skill.reviewVerdict !== "";

  return (
    <Card className="p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="truncate text-[14px] font-semibold tracking-tight text-[var(--fg)]">{skill.name}</span>
            {skill.version ? <Pill mono>v{skill.version}</Pill> : null}
            {isDownloading ? <Pill tone="brand">{t("mySkills.downloading")}</Pill> : null}
            {isError ? <Pill tone="danger">{t("mySkills.downloadError")}</Pill> : null}
            {!isDownloading && !isError && skill.isModified ? (
              <Pill tone="warning">{t("local.modified")}</Pill>
            ) : null}
            {!isDownloading && !isError && hasReview ? (
              <Pill tone={VERDICT_TONE[skill.reviewVerdict] ?? "default"}>
                {t(`mySkills.reviewVerdict.${skill.reviewVerdict}`)}
                {skill.reviewStale ? " ·" : ""}
              </Pill>
            ) : null}
          </div>
          {skill.description ? (
            <div className="mt-0.5 line-clamp-2 text-[12.5px] text-[var(--fg-secondary)]">{skill.description}</div>
          ) : null}
          {skill.sourceWorkspace ? (
            <div className="mt-1 truncate font-mono text-[11px] text-[var(--fg-muted)]">
              ← {skill.sourceWorkspace}{skill.sourceBranch ? `@${skill.sourceBranch}` : ""}
            </div>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {confirmRemove ? (
            <>
              <Button size="sm" variant="outline" onPress={() => setConfirmRemove(false)}>
                {t("common.cancel")}
              </Button>
              <Button size="sm" variant="danger-soft" onPress={onRemove} isPending={removing}>
                {t("mySkills.remove.confirm")}
              </Button>
            </>
          ) : (
            <>
              {!isDownloading && !isError ? (
                <Tooltip delay={150}>
                  <Button
                    size="sm"
                    variant="ghost"
                    className={
                      hasReview
                        ? "text-[var(--fg-muted)] hover:text-[var(--brand)]"
                        : "text-[var(--fg-muted)] hover:text-[var(--brand)]"
                    }
                    isDisabled={!aiConfigured || reviewing}
                    onPress={onReview}
                  >
                    {reviewing ? (
                      <Spinner size="sm" />
                    ) : skill.reviewVerdict === "danger" ? (
                      <ShieldAlert size={14} className="text-[var(--danger)]" />
                    ) : skill.reviewVerdict === "caution" ? (
                      <ShieldAlert size={14} className="text-[var(--warning)]" />
                    ) : skill.reviewVerdict === "safe" ? (
                      <ShieldCheck size={14} className="text-[var(--success)]" />
                    ) : (
                      <ShieldCheck size={14} />
                    )}
                  </Button>
                  <Tooltip.Content>
                    {!aiConfigured
                      ? t("mySkills.aiNotConfigured")
                      : hasReview
                        ? t("mySkills.rereview")
                        : t("mySkills.review")}
                  </Tooltip.Content>
                </Tooltip>
              ) : null}
              <Tooltip delay={150}>
                <Button
                  size="sm"
                  variant="ghost"
                  className="text-[var(--fg-muted)] hover:text-[var(--brand)]"
                  onPress={onPush}
                >
                  <GitPullRequestArrow size={14} />
                </Button>
                <Tooltip.Content>{t("local.pushToWorkspace")}</Tooltip.Content>
              </Tooltip>
              <Button
                size="sm"
                variant="ghost"
                className="text-[var(--fg-muted)] hover:text-[var(--danger)]"
                onPress={() => setConfirmRemove(true)}
              >
                <Trash2 size={14} />
              </Button>
            </>
          )}
        </div>
      </div>

      {isDownloading ? (
        /* Downloading — real percentage bar (indeterminate when progress < 0). */
        <div className="mt-3">
          {skill.downloadProgress >= 0 ? (
            <ProgressBar value={skill.downloadProgress} minValue={0} maxValue={100}>
              <div className="flex items-center justify-between text-[11.5px] text-[var(--fg-muted)]">
                <span>{t("mySkills.downloading")}</span>
                <ProgressBar.Output className="tabular-nums" />
              </div>
              <ProgressBar.Track className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-[var(--bg-soft)]">
                <ProgressBar.Fill className="h-full rounded-full bg-[var(--brand)]" />
              </ProgressBar.Track>
            </ProgressBar>
          ) : (
            <ProgressBar isIndeterminate aria-label={t("mySkills.downloading")}>
              <div className="text-[11.5px] text-[var(--fg-muted)]">{t("mySkills.downloading")}</div>
              <ProgressBar.Track className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-[var(--bg-soft)]">
                <ProgressBar.Fill className="h-full w-1/3 rounded-full bg-[var(--brand)]" />
              </ProgressBar.Track>
            </ProgressBar>
          )}
        </div>
      ) : isError ? (
        /* Failed/interrupted — show the reason and offer a retry. */
        <div className="mt-3 flex items-center justify-between gap-3 rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2">
          <div className="flex min-w-0 items-center gap-2 text-[12px] text-[var(--danger)]">
            <AlertTriangle size={14} className="shrink-0" />
            <span className="truncate">{skill.downloadError || t("mySkills.downloadError")}</span>
          </div>
          <Button size="sm" variant="outline" onPress={onRetry} isPending={retrying}>
            <RotateCcw size={13} />
            {t("mySkills.retry")}
          </Button>
        </div>
      ) : (
        <>
          {/* Per-tool sync toggles */}
          <div className="mt-3 flex flex-wrap gap-2">
            {tools.map((runtime) => {
              const target = skill.targets.find((tg) => tg.runtime === runtime);
              const enabled = target?.enabled ?? false;
              const busy = busyRuntimeKey === `${skill.id}:${runtime}`;
              return (
                <label
                  key={runtime}
                  className={`flex cursor-pointer items-center gap-2 rounded-md border px-2.5 py-1.5 text-[12px] ${
                    enabled
                      ? "border-[var(--brand)] bg-[var(--brand-soft)] text-[var(--brand-fg)]"
                      : "border-[var(--line)] text-[var(--fg-secondary)]"
                  }`}
                >
                  <Switch isSelected={enabled} isDisabled={busy} onChange={() => onToggle(runtime, enabled)}>
                    <Switch.Control>
                      <Switch.Thumb />
                    </Switch.Control>
                  </Switch>
                  {TOOL_LABELS[runtime] ?? runtime}
                </label>
              );
            })}
          </div>

          {/* AI review result (cached) — verdict summary + expandable findings. */}
          {hasReview ? (
            <div className="mt-3 rounded-md border border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2">
              <div className="flex items-start gap-2">
                {skill.reviewVerdict === "safe" ? (
                  <ShieldCheck size={14} className="mt-0.5 shrink-0 text-[var(--success)]" />
                ) : skill.reviewVerdict === "caution" ? (
                  <ShieldAlert size={14} className="mt-0.5 shrink-0 text-[var(--warning)]" />
                ) : (
                  <ShieldAlert size={14} className="mt-0.5 shrink-0 text-[var(--danger)]" />
                )}
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[12.5px] font-medium text-[var(--fg)]">
                      {t(`mySkills.reviewVerdict.${skill.reviewVerdict}`)}
                    </span>
                    {skill.reviewFindings.length ? (
                      <button
                        type="button"
                        className="ml-auto flex items-center gap-0.5 text-[11.5px] text-[var(--fg-muted)] hover:text-[var(--fg)]"
                        onClick={() => setShowFindings((v) => !v)}
                      >
                        {showFindings ? t("mySkills.hideFindings") : t("mySkills.viewFindings")}
                        <ChevronDown
                          size={12}
                          className={showFindings ? "rotate-180 transition-transform" : "transition-transform"}
                        />
                      </button>
                    ) : null}
                  </div>
                  {skill.reviewSummary ? (
                    <div className="mt-0.5 text-[11.5px] leading-[1.5] text-[var(--fg-secondary)]">
                      {skill.reviewSummary}
                    </div>
                  ) : (
                    <div className="mt-0.5 text-[11.5px] text-[var(--fg-muted)]">
                      {t("mySkills.reviewNoFindings")}
                    </div>
                  )}
                  {skill.reviewStale ? (
                    <div className="mt-1 text-[11px] text-[var(--warning)]">{t("mySkills.reviewStale")}</div>
                  ) : null}
                  {showFindings && skill.reviewFindings.length ? (
                    <div className="mt-2 space-y-1.5 border-t border-[var(--line)] pt-2">
                      {skill.reviewFindings.map((f, i) => (
                        <div key={i} className="flex items-start gap-2 text-[11.5px]">
                          <Pill tone={SEVERITY_TONE[f.severity] ?? "default"}>{f.severity}</Pill>
                          <span className="text-[var(--fg-secondary)]">{f.detail}</span>
                        </div>
                      ))}
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
          ) : null}

          {/* Auto-update */}
          <div className="mt-3 flex items-center justify-between border-t border-[var(--line)] pt-3">
            <span className="text-[12.5px] text-[var(--fg-secondary)]">{t("mySkills.autoUpdate")}</span>
            <Switch isSelected={autoUpdate} onChange={(v) => setAutoUpdate(v)}>
              <Switch.Control>
                <Switch.Thumb />
              </Switch.Control>
            </Switch>
          </div>
        </>
      )}
    </Card>
  );
}
