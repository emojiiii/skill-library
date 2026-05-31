import { Button, Modal, Spinner, Switch } from "@heroui/react";
import { ShieldAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import type { SkillManifest } from "../lib/teamai";
import { plainPermissionLines, riskRequiresConfirmation, effectiveRisk } from "../utils/risk";
import { SkillSafetyCard } from "./SkillSafetyCard";

const TOOLS = [
  { id: "claude-code", label: "Claude Code" },
  { id: "cursor", label: "Cursor" },
  { id: "codex", label: "Codex" },
];

/**
 * Consumer one-click install: pick which AI tools to sync this skill into.
 * All three tools default to OFF — selecting none is allowed and simply
 * downloads the skill to local (recorded as downloaded-but-not-deployed; the
 * user can enable a tool later). A plain-language safety card is shown, and for
 * skills that can modify the machine an explicit confirmation gate is required
 * before install — but only when at least one tool is selected.
 */
export function InstallToToolsDialog({
  open,
  onOpenChange,
  manifest,
  loading,
  sourceLabel,
  defaultTargets,
  onConfirm,
  pending,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  manifest: SkillManifest | null;
  /** Detail still loading — show a spinner instead of nothing (cold install). */
  loading?: boolean;
  sourceLabel: string;
  defaultTargets: string[];
  onConfirm: (targets: string[]) => void;
  pending: boolean;
}) {
  const { t } = useLocale();
  const [selected, setSelected] = useState<string[]>(defaultTargets);
  const [acknowledged, setAcknowledged] = useState(false);

  useEffect(() => {
    if (open) {
      // Default to whatever the caller passed (which is now empty = all tools
      // off). Selecting none is intentional — it downloads without deploying.
      setSelected(defaultTargets);
      setAcknowledged(false);
    }
  }, [open, defaultTargets]);

  // Manifest still loading (e.g. installing straight from a card hover before
  // the detail query resolved) — show a small loading dialog so the click has
  // immediate feedback instead of nothing.
  if (!manifest) {
    if (open && loading) {
      return (
        <Modal isOpen={open} onOpenChange={onOpenChange}>
          <Modal.Backdrop>
            <Modal.Container size="md">
              <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
                <Modal.Body className="flex items-center justify-center gap-3 px-5 py-10 text-[13px] text-[var(--fg-muted)]">
                  <Spinner size="sm" />
                  {t("common.loading")}
                </Modal.Body>
              </Modal.Dialog>
            </Modal.Container>
          </Modal.Backdrop>
        </Modal>
      );
    }
    return null;
  }

  const needsConfirm = riskRequiresConfirmation(effectiveRisk(manifest));
  const caps = plainPermissionLines(manifest, t);
  // Selecting no tools is allowed — it just downloads the skill locally without
  // deploying anywhere, so the machine-modification risk gate doesn't apply.
  // The gate only matters once a tool is selected (i.e. files land in an agent
  // dir and could run).
  const downloadOnly = selected.length === 0;
  const canInstall = downloadOnly || !needsConfirm || acknowledged;

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="md">
          <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                {t("install.toTools.title").replace("{name}", manifest.name)}
              </Modal.Heading>
              <div className="mt-1 text-[12px] text-[var(--fg-muted)]">{sourceLabel}</div>
            </Modal.Header>

            <Modal.Body className="space-y-4 px-5 py-4">
              <section>
                <div className="mb-2 text-[12px] font-medium text-[var(--fg-secondary)]">
                  {t("install.toTools.chooseTools")}
                </div>
                <div className="space-y-1.5">
                  {TOOLS.map((tool) => {
                    const enabled = selected.includes(tool.id);
                    return (
                      <label
                        key={tool.id}
                        className="flex cursor-pointer items-center justify-between rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2.5 hover:bg-[var(--bg-soft)]"
                      >
                        <span className="text-[13px] font-medium">{tool.label}</span>
                        <Switch
                          isSelected={enabled}
                          onChange={(value) =>
                            setSelected((prev) =>
                              value
                                ? Array.from(new Set([...prev, tool.id]))
                                : prev.filter((id) => id !== tool.id),
                            )
                          }
                        >
                          <Switch.Control>
                            <Switch.Thumb />
                          </Switch.Control>
                        </Switch>
                      </label>
                    );
                  })}
                </div>
              </section>

              <SkillSafetyCard manifest={manifest} />

              {needsConfirm && !downloadOnly ? (
                <label className="flex cursor-pointer items-start gap-2.5 rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2.5">
                  <input
                    type="checkbox"
                    checked={acknowledged}
                    onChange={(e) => setAcknowledged(e.target.checked)}
                    className="mt-0.5 size-4 shrink-0 accent-[var(--warning)]"
                  />
                  <span className="flex items-start gap-1.5 text-[12.5px] leading-[1.5] text-[var(--warning)]">
                    <ShieldAlert size={14} className="mt-0.5 shrink-0" />
                    <span>
                      {t("install.toTools.confirm")}
                      {caps.length ? ` ${caps.join("、")}。` : ""}
                    </span>
                  </span>
                </label>
              ) : null}
            </Modal.Body>

            <div className="flex items-center justify-between gap-2 border-t border-[var(--line)] px-5 py-3">
              <span className="text-[11.5px] text-[var(--fg-muted)]">
                {downloadOnly ? t("install.toTools.downloadOnlyHint") : ""}
              </span>
              <div className="flex shrink-0 gap-2">
                <Button variant="outline" onPress={() => onOpenChange(false)}>
                  {t("common.cancel")}
                </Button>
                <Button
                  onPress={() => onConfirm(selected)}
                  isPending={pending}
                  isDisabled={!canInstall}
                >
                  {downloadOnly ? t("install.toTools.downloadOnly") : t("install.toTools.action")}
                </Button>
              </div>
            </div>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
