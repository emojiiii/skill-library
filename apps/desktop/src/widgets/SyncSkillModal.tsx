import { Button, Input, Modal } from "@heroui/react";
import { useMutation } from "@tanstack/react-query";
import { ExternalLink, GitPullRequestArrow, ShieldAlert } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import {
  type PublishPreview,
  type PublishResult,
  type StoredWorkspace,
  previewPublishFromWorkspace,
  publishSkillToWorkspace,
} from "../lib/skill-library";
import { openExternalUrl } from "../utils/format";
import { workspaceColor, workspaceInitials } from "../utils/workspace-visual";
import { Pill, type PillTone } from "./Pill";

const decisionTone: Record<string, PillTone> = {
  allow_auto_merge: "success",
  require_review: "warning",
  reject: "danger",
};

const writeRoles = new Set(["admin", "maintain", "write"]);

export function SyncSkillModal({
  open,
  onOpenChange,
  sourceWorkspace,
  skillPath,
  skillId,
  sourceRef,
  workspaces,
  authLogin,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sourceWorkspace: string;
  skillPath: string;
  skillId: string;
  sourceRef?: string;
  workspaces: StoredWorkspace[];
  authLogin?: string | null;
}) {
  const { t } = useLocale();
  const eligibleWorkspaces = useMemo(
    () =>
      workspaces.filter(
        (ws) => writeRoles.has(ws.permission) && ws.full_name !== sourceWorkspace,
      ),
    [workspaces, sourceWorkspace],
  );

  const [target, setTarget] = useState("");
  const [renameTo, setRenameTo] = useState("");
  const [preview, setPreview] = useState<PublishPreview | null>(null);
  const [result, setResult] = useState<PublishResult | null>(null);

  const previewMutation = useMutation({
    mutationFn: previewPublishFromWorkspace,
    onSuccess: (data) => setPreview(data),
  });

  const publishMutation = useMutation({
    mutationFn: publishSkillToWorkspace,
    onSuccess: (data) => setResult(data),
  });

  // Reset state when opened.
  useEffect(() => {
    if (open) {
      setTarget(eligibleWorkspaces[0]?.full_name ?? "");
      setRenameTo("");
      setPreview(null);
      setResult(null);
      previewMutation.reset();
      publishMutation.reset();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Auto-preview when target / rename changes.
  useEffect(() => {
    if (!open || !target) return;
    setResult(null);
    previewMutation.mutate({
      sourceWorkspace,
      skillPath,
      sourceRef,
      targetWorkspace: target,
      renameTo: renameTo.trim() || undefined,
      user: authLogin ?? "local",
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, target, renameTo, sourceWorkspace, skillPath, sourceRef]);

  const policy = preview?.policy;
  const requiresConfirmation =
    !!policy && policy.risk_level !== "low" && policy.decision !== "reject";
  const blocked = !!policy && policy.decision === "reject";
  const previewError = previewMutation.error
    ? previewMutation.error instanceof Error
      ? previewMutation.error.message
      : String(previewMutation.error)
    : null;
  const publishError = publishMutation.error
    ? publishMutation.error instanceof Error
      ? publishMutation.error.message
      : String(publishMutation.error)
    : null;

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="md">
          <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
            <Modal.CloseTrigger />
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                {t("sync.title")} &mdash; &ldquo;{skillId}&rdquo;
              </Modal.Heading>
              <div className="mt-1 truncate text-[12px] text-[var(--fg-muted)]">
                {t("sync.source")} <span className="font-mono">{sourceWorkspace}</span>
                {sourceRef ? (
                  <>
                    {" · "}
                    <span className="font-mono">{sourceRef}</span>
                  </>
                ) : null}
                {" · "}
                <span className="font-mono">{skillPath}</span>
              </div>
            </Modal.Header>

            <Modal.Body className="space-y-5 px-5 py-4">
              {/* Target workspace */}
              <section>
                <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                  {t("sync.targetWorkspace")}
                </div>
                {eligibleWorkspaces.length ? (
                  <div className="space-y-1">
                    {eligibleWorkspaces.map((ws) => {
                      const active = target === ws.full_name;
                      const color = workspaceColor(ws.full_name);
                      const initials = workspaceInitials({
                        owner: ws.full_name.split("/")[0] ?? "",
                        repo: ws.full_name.split("/")[1] ?? "",
                      });
                      return (
                        <button
                          key={ws.full_name}
                          type="button"
                          onClick={() => setTarget(ws.full_name)}
                          className={`flex w-full items-center gap-3 rounded-md border px-3 py-2 text-left transition-colors ${
                            active
                              ? "border-[var(--brand)] bg-[var(--brand-soft)]"
                              : "border-[var(--line)] bg-[var(--bg-elevated)] hover:bg-[var(--bg-soft)]"
                          }`}
                        >
                          <span
                            className="grid size-7 place-items-center rounded-md text-[10px] font-semibold"
                            style={{ background: color.bg, color: color.fg }}
                          >
                            {initials}
                          </span>
                          <span className="min-w-0 flex-1">
                            <div className="truncate text-[13px] font-medium">{ws.full_name}</div>
                            <div className="text-[11px] text-[var(--fg-muted)]">
                              {ws.permission} · {ws.visibility}
                            </div>
                          </span>
                        </button>
                      );
                    })}
                  </div>
                ) : (
                  <div className="rounded-md border border-dashed border-[var(--line)] px-3 py-6 text-center text-[12px] text-[var(--fg-muted)]">
                    {t("sync.noWriteAccess")}
                  </div>
                )}
              </section>

              {/* Rename */}
              <section>
                <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                  {t("sync.skillIdOptional")}
                </div>
                <Input
                  aria-label={t("sync.renameAria")}
                  value={renameTo}
                  onChange={(event) => setRenameTo(event.target.value)}
                  placeholder={skillId}
                  variant="secondary"
                />
                <div className="mt-1 text-[11px] text-[var(--fg-muted)]">
                  {t("sync.renameKeep").replace("{id}", skillId)}
                </div>
              </section>

              {/* Preview */}
              <section>
                <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                  {t("sync.previewRisk")}
                </div>
                {previewMutation.isPending ? (
                  <div className="rounded-md border border-[var(--line)] px-3 py-3 text-[12.5px] text-[var(--fg-muted)]">
                    {t("sync.fetchingFiles").replace("{workspace}", sourceWorkspace)}
                  </div>
                ) : preview && policy ? (
                  <div className="rounded-md border border-[var(--line)] bg-[var(--bg-soft)]">
                    <div className="flex items-center justify-between gap-3 border-b border-[var(--line)] px-3 py-2">
                      <div className="flex items-center gap-2">
                        <ShieldAlert size={14} className="text-[var(--warning)]" />
                        <span className="text-[12.5px] font-medium">
                          {t("sync.riskLabel").replace("{risk}", t(`risk.level.${policy.risk_level}`))}
                        </span>
                      </div>
                      <Pill tone={decisionTone[policy.decision] ?? "default"}>
                        {policy.decision.replaceAll("_", " ")}
                      </Pill>
                    </div>
                    <div className="grid grid-cols-2 gap-3 px-3 py-3 text-[12px]">
                      <div>
                        <div className="text-[10.5px] uppercase tracking-wider text-[var(--fg-muted)]">{t("common.files")}</div>
                        <div className="mt-0.5 font-medium">{preview.package.file_count}</div>
                      </div>
                      <div>
                        <div className="text-[10.5px] uppercase tracking-wider text-[var(--fg-muted)]">{t("common.size")}</div>
                        <div className="mt-0.5 font-medium">
                          {(preview.package.total_bytes / 1024).toFixed(1)} KB
                        </div>
                      </div>
                      {policy.reasons.length ? (
                        <div className="col-span-2">
                          <div className="text-[10.5px] uppercase tracking-wider text-[var(--fg-muted)]">{t("common.reasons")}</div>
                          <ul className="mt-1 list-disc pl-4 text-[11.5px] text-[var(--fg-secondary)]">
                            {policy.reasons.map((reason) => (
                              <li key={reason}>{reason}</li>
                            ))}
                          </ul>
                        </div>
                      ) : null}
                    </div>
                    {preview.request ? (
                      <div className="border-t border-[var(--line)] px-3 py-2.5">
                        <div className="text-[10.5px] uppercase tracking-wider text-[var(--fg-muted)]">{t("common.prDraft")}</div>
                        <div className="mt-0.5 truncate text-[12.5px] font-medium">{preview.request.title}</div>
                        <div className="truncate text-[11.5px] font-mono text-[var(--fg-muted)]">
                          {t("common.branch")}: {preview.request.branch_name}
                        </div>
                      </div>
                    ) : null}
                  </div>
                ) : previewError ? (
                  <div className="rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
                    {previewError}
                  </div>
                ) : (
                  <div className="rounded-md border border-dashed border-[var(--line)] px-3 py-4 text-center text-[12px] text-[var(--fg-muted)]">
                    {t("sync.pickWorkspacePreview")}
                  </div>
                )}
              </section>

              {publishError ? (
                <div className="rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
                  {publishError}
                </div>
              ) : null}

              {result ? (
                <div className="rounded-md border border-[var(--success)] bg-[var(--success-soft)] px-3 py-2.5 text-[12.5px] text-[var(--success)]">
                  <div className="font-semibold">
                    {result.autoMerge?.merged ? t("sync.prAutoMerged") : t("sync.prOpened")}
                  </div>
                  <div className="mt-0.5 truncate text-[11.5px] opacity-80">
                    #{result.pullRequest.number} · {result.pullRequest.title}
                  </div>
                  {result.autoMerge?.error ? (
                    <div className="mt-1 rounded border border-[var(--warning)] bg-[var(--warning-soft)] px-2 py-1 text-[11.5px] text-[var(--warning)]">
                      {t("sync.autoMergeNotice").replace("{error}", result.autoMerge.error)}
                    </div>
                  ) : null}
                  <button
                    type="button"
                    onClick={() => void openExternalUrl(result.pullRequest.htmlUrl)}
                    className="mt-2 inline-flex items-center gap-1 text-[12px] font-medium underline"
                  >
                    <ExternalLink size={12} />
                    {t("sync.openOnGithub")}
                  </button>
                </div>
              ) : null}
            </Modal.Body>

            <div className="flex justify-end gap-2 border-t border-[var(--line)] px-5 py-3">
              <Button variant="outline" onPress={() => onOpenChange(false)}>
                {result ? t("sync.close") : t("sync.cancel")}
              </Button>
              {!result ? (
                <Button
                  onPress={() =>
                    publishMutation.mutate({
                      sourceWorkspace,
                      skillPath,
                      sourceRef,
                      targetWorkspace: target,
                      renameTo: renameTo.trim() || undefined,
                      user: authLogin ?? "local",
                      confirmedRisk: requiresConfirmation,
                    })
                  }
                  isPending={publishMutation.isPending}
                  isDisabled={!target || !preview || previewMutation.isPending || blocked}
                >
                  <GitPullRequestArrow size={14} />
                  {requiresConfirmation ? t("sync.confirmOpenPr") : t("sync.openPr")}
                </Button>
              ) : null}
            </div>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
