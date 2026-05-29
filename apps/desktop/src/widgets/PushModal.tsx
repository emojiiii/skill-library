import { Button, Modal } from "@heroui/react";
import { Check, ChevronsUpDown, GitPullRequestArrow, ShieldAlert } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { LocalAgentEntry, PublishPreview, StoredWorkspace } from "../lib/teamai";
import { Pill, type PillTone } from "../widgets/Pill";
import { riskLabel } from "../utils/risk";
import { workspaceColor, workspaceInitials } from "../utils/workspace-visual";
import { useLocale } from "../hooks/useLocale";

const decisionTone: Record<string, PillTone> = {
  allow_auto_merge: "success",
  require_review: "warning",
  reject: "danger",
};

export function PushModal({
  open,
  onOpenChange,
  entry,
  workspaces,
  preview,
  previewPending,
  onPreview,
  onConfirm,
  confirmPending,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  entry: LocalAgentEntry | null;
  workspaces: StoredWorkspace[];
  preview: PublishPreview | null;
  previewPending: boolean;
  onPreview: (input: { source: string; workspace: string }) => void;
  onConfirm: () => void;
  confirmPending: boolean;
}) {
  const { t } = useLocale();
  const [target, setTarget] = useState("");
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);
  // Keep last valid preview to avoid flash during loading
  const lastPreviewRef = useRef<PublishPreview | null>(null);
  if (preview) lastPreviewRef.current = preview;

  useEffect(() => {
    if (open) {
      setTarget(workspaces[0]?.full_name ?? "");
      lastPreviewRef.current = null;
    }
  }, [open, workspaces]);

  useEffect(() => {
    if (open && entry && target) {
      onPreview({ source: entry.path, workspace: target });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, target, entry?.path]);

  // Close dropdown on outside click
  useEffect(() => {
    if (!dropdownOpen) return;
    const onDocClick = (event: MouseEvent) => {
      if (!dropdownRef.current?.contains(event.target as Node)) setDropdownOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setDropdownOpen(false);
    };
    document.addEventListener("mousedown", onDocClick);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDocClick);
      document.removeEventListener("keydown", onKey);
    };
  }, [dropdownOpen]);

  if (!entry) return null;

  const policy = preview?.policy ?? lastPreviewRef.current?.policy;
  const displayPreview = preview ?? lastPreviewRef.current;
  const selectedWs = workspaces.find((ws) => ws.full_name === target);
  const selectedColor = selectedWs ? workspaceColor(selectedWs.full_name) : { bg: "var(--bg-active)", fg: "var(--fg-muted)" };
  const selectedInitials = selectedWs
    ? workspaceInitials({ owner: selectedWs.full_name.split("/")[0] ?? "", repo: selectedWs.full_name.split("/")[1] ?? "" })
    : "-";

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="md">
          <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                Push &ldquo;{entry.name}&rdquo; to a workspace
              </Modal.Heading>
              <div className="mt-1 truncate text-[12px] font-mono text-[var(--fg-muted)]">{entry.path}</div>
            </Modal.Header>

            <Modal.Body className="space-y-5 px-5 py-4">
              <section>
                <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                  Target workspace
                </div>
                {workspaces.length ? (
                  <div ref={dropdownRef} className="relative">
                    <button
                      type="button"
                      onClick={() => setDropdownOpen((v) => !v)}
                      className="flex w-full items-center gap-3 rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2.5 text-left transition-colors hover:bg-[var(--bg-soft)]"
                    >
                      <span
                        className="grid size-7 shrink-0 place-items-center rounded-md text-[10px] font-semibold"
                        style={{ background: selectedColor.bg, color: selectedColor.fg }}
                      >
                        {selectedInitials}
                      </span>
                      <span className="min-w-0 flex-1">
                        <div className="truncate text-[13px] font-medium">{target || "Select workspace"}</div>
                        {selectedWs && (
                          <div className="text-[11px] text-[var(--fg-muted)]">
                            {selectedWs.permission} &middot; {selectedWs.visibility}
                          </div>
                        )}
                      </span>
                      <ChevronsUpDown size={14} className="shrink-0 text-[var(--fg-muted)]" />
                    </button>

                    {dropdownOpen && (
                      <div className="absolute left-0 right-0 top-full z-50 mt-1 max-h-[200px] overflow-y-auto rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] shadow-lg">
                        {workspaces.map((ws) => {
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
                              onClick={() => {
                                setTarget(ws.full_name);
                                setDropdownOpen(false);
                              }}
                              className={`flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-[var(--bg-soft)] ${
                                active ? "bg-[var(--brand-soft)]" : ""
                              }`}
                            >
                              <span
                                className="grid size-6 shrink-0 place-items-center rounded-md text-[9px] font-semibold"
                                style={{ background: color.bg, color: color.fg }}
                              >
                                {initials}
                              </span>
                              <span className="min-w-0 flex-1">
                                <div className="truncate text-[13px] font-medium">{ws.full_name}</div>
                                <div className="text-[11px] text-[var(--fg-muted)]">
                                  {ws.permission} &middot; {ws.visibility}
                                </div>
                              </span>
                              {active && <Check size={13} className="shrink-0 text-[var(--brand)]" />}
                              {!["admin", "maintain", "write"].includes(ws.permission) && (
                                <Pill tone="warning">read-only</Pill>
                              )}
                            </button>
                          );
                        })}
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="rounded-md border border-dashed border-[var(--line)] px-3 py-6 text-center text-[12px] text-[var(--fg-muted)]">
                    Add a workspace before publishing.
                  </div>
                )}
              </section>

              {/* Risk preview */}
              <section>
                <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                  Risk &amp; policy preview
                </div>
                {previewPending && !displayPreview ? (
                  <div className="rounded-md border border-[var(--line)] px-3 py-3 text-[12.5px] text-[var(--fg-muted)]">
                    Generating preview...
                  </div>
                ) : displayPreview && policy ? (
                  <div className={`rounded-md border border-[var(--line)] bg-[var(--bg-soft)] transition-opacity ${previewPending ? "opacity-50" : ""}`}>
                    <div className="flex items-center justify-between gap-3 border-b border-[var(--line)] px-3 py-2">
                      <div className="flex items-center gap-2">
                        <ShieldAlert size={14} className="text-[var(--warning)]" />
                        <span className="text-[12.5px] font-medium">{riskLabel[policy.risk_level]} risk</span>
                      </div>
                      <Pill tone={decisionTone[policy.decision] ?? "default"}>
                        {policy.decision.replaceAll("_", " ")}
                      </Pill>
                    </div>
                    <div className="grid grid-cols-2 gap-3 px-3 py-3 text-[12px]">
                      <div>
                        <div className="text-[10.5px] uppercase tracking-wider text-[var(--fg-muted)]">Files</div>
                        <div className="mt-0.5 font-medium">{displayPreview.package.file_count}</div>
                      </div>
                      <div>
                        <div className="text-[10.5px] uppercase tracking-wider text-[var(--fg-muted)]">Size</div>
                        <div className="mt-0.5 font-medium">
                          {(displayPreview.package.total_bytes / 1024).toFixed(1)} KB
                        </div>
                      </div>
                      <div className="col-span-2">
                        <div className="text-[10.5px] uppercase tracking-wider text-[var(--fg-muted)]">Hash</div>
                        <div className="mt-0.5 truncate font-mono text-[11px]">{displayPreview.package.source_hash}</div>
                      </div>
                      {policy.reasons.length ? (
                        <div className="col-span-2">
                          <div className="text-[10.5px] uppercase tracking-wider text-[var(--fg-muted)]">Reasons</div>
                          <ul className="mt-1 list-disc pl-4 text-[11.5px] text-[var(--fg-secondary)]">
                            {policy.reasons.map((reason) => (
                              <li key={reason}>{reason}</li>
                            ))}
                          </ul>
                        </div>
                      ) : null}
                    </div>
                    {displayPreview.request ? (
                      <div className="border-t border-[var(--line)] px-3 py-2.5">
                        <div className="text-[10.5px] uppercase tracking-wider text-[var(--fg-muted)]">PR draft</div>
                        <div className="mt-0.5 truncate text-[12.5px] font-medium">{displayPreview.request.title}</div>
                        <div className="truncate text-[11.5px] font-mono text-[var(--fg-muted)]">
                          branch: {displayPreview.request.branch_name}
                        </div>
                      </div>
                    ) : null}
                  </div>
                ) : (
                  <div className="rounded-md border border-dashed border-[var(--line)] px-3 py-4 text-center text-[12px] text-[var(--fg-muted)]">
                    Preview will appear once a workspace is selected.
                  </div>
                )}
              </section>
            </Modal.Body>

            <div className="flex justify-end gap-2 border-t border-[var(--line)] px-5 py-3">
              <Button variant="outline" onPress={() => onOpenChange(false)}>
                Cancel
              </Button>
              <Button
                onPress={onConfirm}
                isPending={confirmPending}
                isDisabled={!target || !preview || previewPending}
              >
                <GitPullRequestArrow size={14} />
                Open PR
              </Button>
            </div>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
