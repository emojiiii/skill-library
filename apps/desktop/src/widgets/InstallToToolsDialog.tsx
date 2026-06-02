import { Button, Modal, Spinner, Switch, toast } from "@heroui/react";
import { FolderOpen, ShieldAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import { selectProjectDirectory, type ProjectInstallTarget, type SkillManifest } from "../lib/skill-library";
import { plainPermissionLines, riskRequiresConfirmation, effectiveRisk } from "../utils/risk";
import { SkillSafetyCard } from "./SkillSafetyCard";

const TOOLS = [
  { id: "claude-code", label: "Claude Code" },
  { id: "codex", label: "Codex" },
];
const TOOL_IDS = new Set(TOOLS.map((tool) => tool.id));

type InstallScope = "global" | "project";

export interface InstallTargetSelection {
  targets: string[];
  projectTargets: ProjectInstallTarget[];
}

function normalizeToolTargets(targets: string[]): string[] {
  return targets.filter((target, index) => TOOL_IDS.has(target) && targets.indexOf(target) === index);
}

/**
 * Consumer one-click install: pick which AI tools to sync this skill into.
 * Tools default to OFF — selecting none is allowed and simply
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
  fallbackName,
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
  fallbackName?: string;
  defaultTargets: string[];
  onConfirm: (selection: InstallTargetSelection) => void;
  pending: boolean;
}) {
  const { t } = useLocale();
  const [selected, setSelected] = useState<string[]>(defaultTargets);
  const [scope, setScope] = useState<InstallScope>("global");
  const [projectRoot, setProjectRoot] = useState("");
  const [acknowledged, setAcknowledged] = useState(false);

  useEffect(() => {
    if (open) {
      // Default to whatever the caller passed (which is now empty = all tools
      // off). Selecting none is intentional — it downloads without deploying.
      setSelected(normalizeToolTargets(defaultTargets));
      setScope("global");
      setProjectRoot("");
      setAcknowledged(false);
    }
  }, [open, defaultTargets]);

  const setInstallScope = (next: InstallScope) => {
    const fallbackTargets = normalizeToolTargets(defaultTargets);
    setScope(next);
    setSelected(
      next === "project"
        ? [selected.find((id) => TOOL_IDS.has(id)) ?? fallbackTargets[0] ?? "codex"]
        : fallbackTargets,
    );
  };

  const chooseProjectRoot = async () => {
    try {
      const selected = await selectProjectDirectory();
      if (selected) setProjectRoot(selected);
    } catch (err) {
      toast.danger(String((err as { message?: string })?.message ?? err));
    }
  };

  // Manifest still loading (e.g. installing straight from a card hover before
  // the detail query resolved) — show a small loading dialog so the click has
  // immediate feedback instead of nothing.
  if (!manifest) {
    if (open && loading && !fallbackName) {
      return (
        <Modal isOpen={open} onOpenChange={onOpenChange}>
          <Modal.Backdrop>
            <Modal.Container size="md">
              <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
                <Modal.CloseTrigger />
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
  }

  const displayName = manifest?.name ?? fallbackName ?? sourceLabel;
  const needsConfirm = manifest ? riskRequiresConfirmation(effectiveRisk(manifest)) : false;
  const caps = manifest ? plainPermissionLines(manifest, t) : [];
  // Selecting no tools is allowed — it just downloads the skill locally without
  // deploying anywhere, so the machine-modification risk gate doesn't apply.
  // The gate only matters once a tool is selected (i.e. files land in an agent
  // dir and could run).
  const selectedTargets = normalizeToolTargets(selected);
  const downloadOnly = scope === "global" && selectedTargets.length === 0;
  const projectRuntime = selectedTargets[0] ?? normalizeToolTargets(defaultTargets)[0] ?? "codex";
  const hasDeploymentTarget = scope === "global" ? selected.length > 0 : Boolean(projectRoot.trim());
  const riskAllowed = downloadOnly || !needsConfirm || acknowledged;
  const canInstall = Boolean((downloadOnly || hasDeploymentTarget) && riskAllowed);
  const projectTargets =
    scope === "project" && projectRoot.trim()
      ? [{ runtime: projectRuntime, projectRoot: projectRoot.trim() }]
      : [];

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="md">
          <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
            <Modal.CloseTrigger />
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                {t("install.toTools.title").replace("{name}", displayName)}
              </Modal.Heading>
              <div className="mt-1 text-[12px] text-[var(--fg-muted)]">{sourceLabel}</div>
            </Modal.Header>

            <Modal.Body className="space-y-4 px-5 py-4">
              <section>
                <div className="mb-2 text-[12px] font-medium text-[var(--fg-secondary)]">
                  {t("install.scope")}
                </div>
                <div className="grid grid-cols-2 gap-2">
                  {(["global", "project"] as InstallScope[]).map((value) => (
                    <button
                      key={value}
                      type="button"
                      onClick={() => setInstallScope(value)}
                      className={`rounded-md border px-3 py-2 text-left transition-colors ${
                        scope === value
                          ? "border-[var(--brand)] bg-[var(--brand-soft)] text-[var(--brand-fg)]"
                          : "border-[var(--line)] bg-[var(--bg-elevated)] text-[var(--fg)] hover:bg-[var(--bg-soft)]"
                      }`}
                    >
                      <div className="text-[12.5px] font-medium">{t(`install.scope.${value}`)}</div>
                      <div className="mt-0.5 text-[11px] text-[var(--fg-muted)]">
                        {t(`install.scope.${value}.desc`)}
                      </div>
                    </button>
                  ))}
                </div>
              </section>

              {scope === "project" ? (
                <section>
                  <div className="mb-2 text-[12px] font-medium text-[var(--fg-secondary)]">
                    {t("install.projectRoot")}
                  </div>
                  <div className="flex items-center gap-2">
                    <input
                      value={projectRoot}
                      onChange={(event) => setProjectRoot(event.target.value)}
                      placeholder={t("install.projectRoot.placeholder")}
                      className="min-w-0 flex-1 rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2 font-mono text-[12px] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                    />
                    <Button size="sm" variant="outline" onPress={chooseProjectRoot}>
                      <FolderOpen size={13} />
                      {t("local.chooseFolder")}
                    </Button>
                  </div>
                  <div className="mt-1.5 text-[11px] text-[var(--fg-muted)]">
                    {t("install.projectRoot.desc")}
                  </div>
                </section>
              ) : null}

              <section>
                <div className="mb-2 text-[12px] font-medium text-[var(--fg-secondary)]">
                  {scope === "project" ? t("install.projectRuntime") : t("install.toTools.chooseTools")}
                </div>
                {scope === "project" ? (
                  <div className="grid grid-cols-2 gap-2">
                    {TOOLS.map((tool) => {
                      const enabled = projectRuntime === tool.id;
                      return (
                        <button
                          key={tool.id}
                          type="button"
                          onClick={() => setSelected([tool.id])}
                          className={`rounded-md border px-3 py-2 text-left text-[12.5px] transition-colors ${
                            enabled
                              ? "border-[var(--brand)] bg-[var(--brand-soft)] text-[var(--brand-fg)]"
                              : "border-[var(--line)] bg-[var(--bg-elevated)] text-[var(--fg)] hover:bg-[var(--bg-soft)]"
                          }`}
                        >
                          {tool.label}
                        </button>
                      );
                    })}
                  </div>
                ) : (
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
                )}
              </section>

              {manifest ? <SkillSafetyCard manifest={manifest} /> : null}

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
                  onPress={() =>
                    onConfirm({
                      targets: scope === "global" ? selectedTargets : [],
                      projectTargets,
                    })
                  }
                  isPending={pending}
                  isDisabled={!canInstall}
                >
                  {downloadOnly
                    ? t("install.toTools.downloadOnly")
                    : scope === "project"
                      ? t("install.toTools.projectAction")
                      : t("install.toTools.action")}
                </Button>
              </div>
            </div>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
