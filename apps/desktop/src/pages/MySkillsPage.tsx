import { AlertDialog, Button, cn, Modal, ProgressBar, Spinner, Switch, Tooltip, toast } from "@heroui/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, ChevronDown, Download, ExternalLink, FolderOpen, FolderPlus, GitPullRequestArrow, Package, PackageOpen, RefreshCw, RotateCcw, ShieldAlert, ShieldCheck, Trash2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  type AiReviewResult,
  listPathOpeners,
  dbAddProjectDeployments,
  dbCheckProjectDeployments,
  dbDeleteProjectDeployment,
  dbDisableSkill,
  dbEnableSkill,
  dbImportSkill,
  dbListRuntimes,
  dbListSkills,
  dbScanUnmanaged,
  dbSetProjectDeploymentEnabled,
  dbUnmanageSkill,
  downloadSkillAsync,
  type ManagedSkill,
  type ManagedSkillProjectDeployment,
  onSkillDownloadProgress,
  openLocalPath,
  type PathOpener,
  reviewLocalSkill,
  selectProjectDirectory,
  selectSkillDirectory,
  syncNow,
  type UnmanagedSkillInfo,
} from "../lib/skill-library";
import { openExternalUrl } from "../utils/format";
import { useLocale } from "../hooks/useLocale";
import { useLocalStorage } from "../hooks/useLocalStorage";
import { useTheme } from "../hooks/useTheme";
import { useAppStore } from "../state/appStore";
import { Card } from "../widgets/Card";
import { Pill, type PillTone } from "../widgets/Pill";

const TOOL_LABELS: Record<string, string> = {
  "claude-code": "Claude Code",
  codex: "Codex",
};
const INSTALL_TARGET_IDS = new Set(Object.keys(TOOL_LABELS));

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

const EMPTY_IMPORT_GROUPS: Record<string, boolean> = {};

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
  const { t, locale } = useLocale();
  const queryClient = useQueryClient();
  const [showImport, setShowImport] = useState(false);
  const [customSkillPath, setCustomSkillPath] = useState("");
  const [customProjectInstall, setCustomProjectInstall] = useState(false);
  const [customProjectPath, setCustomProjectPath] = useState("");
  const [customProjectRuntime, setCustomProjectRuntime] = useState("codex");
  const [projectInstallSkill, setProjectInstallSkill] = useState<ManagedSkill | null>(null);
  const [projectInstallPath, setProjectInstallPath] = useState("");
  const [projectInstallRuntime, setProjectInstallRuntime] = useState("codex");
  const [openImportGroups, setOpenImportGroups] = useLocalStorage<Record<string, boolean>>(
    "my-skills:import-groups",
    EMPTY_IMPORT_GROUPS,
  );

  // Push-to-workspace uses the global PushModal (lives in RootLayout).
  const setPushEntry = useAppStore((s) => s.setPushEntry);
  const setPushPreview = useAppStore((s) => s.setPushPreview);
  const setPushOpen = useAppStore((s) => s.setPushOpen);

  // AI provider config (same Settings the discover-page review uses).
  const settings = useTheme();
  const activeAiConfig = settings.aiProvider !== "none" ? settings.aiConfigs[settings.aiProvider] : null;
  const aiConfigured = settings.aiProvider !== "none" && Boolean(activeAiConfig?.baseUrl);

  const skills = useQuery({ queryKey: ["db-skills"], queryFn: dbListSkills, staleTime: 30 * 1000 });
  const runtimes = useQuery({ queryKey: ["db-runtimes"], queryFn: dbListRuntimes, staleTime: 5 * 60 * 1000 });
  const pathOpeners = useQuery({ queryKey: ["path-openers"], queryFn: listPathOpeners, staleTime: 10 * 60 * 1000 });
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

  useEffect(() => {
    const id = window.setTimeout(() => {
      void dbCheckProjectDeployments()
        .then((changed) => {
          if (changed > 0) queryClient.invalidateQueries({ queryKey: ["db-skills"] });
        })
        .catch(() => undefined);
    }, 900);
    return () => window.clearTimeout(id);
  }, [queryClient]);

  const enable = useMutation({
    mutationFn: (a: { skillId: string; runtime: string }) => dbEnableSkill(a.skillId, a.runtime),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
  });
  const disable = useMutation({
    mutationFn: (a: { skillId: string; runtime: string }) => dbDisableSkill(a.skillId, a.runtime),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
  });
  const toggleProjectDeployment = useMutation({
    mutationFn: (a: { deploymentId: number; enabled: boolean }) =>
      dbSetProjectDeploymentEnabled(a.deploymentId, a.enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
    onError: (err) => toast.danger(String((err as { message?: string })?.message ?? err)),
  });
  const deleteProjectDeployment = useMutation({
    mutationFn: (deploymentId: number) => dbDeleteProjectDeployment(deploymentId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
    onError: (err) => toast.danger(String((err as { message?: string })?.message ?? err)),
  });
  const addProjectDeployment = useMutation({
    mutationFn: (a: { skillId: string; runtime: string; projectRoot: string }) =>
      dbAddProjectDeployments(a.skillId, [{ runtime: a.runtime, projectRoot: a.projectRoot }]),
    onSuccess: () => {
      setProjectInstallSkill(null);
      setProjectInstallPath("");
      queryClient.invalidateQueries({ queryKey: ["db-skills"] });
      toast.info(t("mySkills.projectAdded"));
    },
    onError: (err) => toast.danger(String((err as { message?: string })?.message ?? err)),
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
    mutationFn: (a: {
      skillId: string;
      sourcePath: string;
      projectTargets?: Array<{ runtime: string; projectRoot: string }>;
    }) => dbImportSkill(a.skillId, a.sourcePath, undefined, a.projectTargets),
    onSuccess: () => {
      setCustomSkillPath("");
      setCustomProjectPath("");
      queryClient.invalidateQueries({ queryKey: ["db-skills"] });
      queryClient.invalidateQueries({ queryKey: ["db-unmanaged"] });
    },
    onError: (err) => toast.danger(String((err as { message?: string })?.message ?? err)),
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
        targets: skill.targets
          .filter((tg) => tg.enabled && INSTALL_TARGET_IDS.has(tg.runtime))
          .map((tg) => tg.runtime),
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
        baseUrl: activeAiConfig?.baseUrl ?? "",
        model: activeAiConfig?.model ?? "",
        language: locale === "zh" ? "zh-CN" : "en",
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
  const projectRuntimeChoices = tools.length ? tools.map((tool) => tool.id) : ["codex", "claude-code"];
  const totalEnabled = list.reduce(
    (sum, s) => sum + s.targets.filter((tg) => tg.enabled && INSTALL_TARGET_IDS.has(tg.runtime)).length,
    0,
  );
  const runtimeMetaById = useMemo(() => {
    const map = new Map<string, { label: string; globalPath: string; exists: boolean }>();
    for (const runtime of runtimes.data ?? []) {
      map.set(runtime.id, {
        label: runtime.label,
        globalPath: runtime.globalPath,
        exists: runtime.exists,
      });
    }
    return map;
  }, [runtimes.data]);

  const busyRuntimeKey = enable.isPending
    ? `${enable.variables?.skillId}:${enable.variables?.runtime}`
    : disable.isPending
      ? `${disable.variables?.skillId}:${disable.variables?.runtime}`
      : null;
  const busyProjectDeploymentId = toggleProjectDeployment.isPending
    ? toggleProjectDeployment.variables?.deploymentId
    : null;

  // Group unmanaged skills by source runtime for the import modal.
  const groupedUnmanaged = useMemo(() => {
    const items = unmanaged.data ?? [];
    const groups = new Map<
      string,
      {
        runtime: string;
        label: string;
        globalPath: string;
        items: Array<UnmanagedSkillInfo & { sourcePath: string }>;
      }
    >();
    for (const skill of items) {
      const locations = skill.locations?.length
        ? skill.locations
        : skill.foundIn.map((runtime) => ({ runtime, path: skill.path }));
      for (const location of locations) {
        const runtime = location.runtime;
        const meta = runtimeMetaById.get(runtime);
        const group = groups.get(runtime) ?? {
          runtime,
          label: meta?.label ?? TOOL_LABELS[runtime] ?? runtime,
          globalPath: meta?.globalPath ?? "",
          items: [],
        };
        group.items.push({ ...skill, sourcePath: location.path });
        groups.set(runtime, group);
      }
    }
    return [...groups.values()].sort((a, b) => a.label.localeCompare(b.label));
  }, [runtimeMetaById, unmanaged.data]);

  const handleChooseSkillFolder = async () => {
    try {
      const selected = await selectSkillDirectory();
      if (selected) setCustomSkillPath(selected);
    } catch (err) {
      toast.danger(String((err as { message?: string })?.message ?? err));
    }
  };

  const handleChooseProjectFolder = async () => {
    try {
      const selected = await selectProjectDirectory();
      if (selected) setCustomProjectPath(selected);
    } catch (err) {
      toast.danger(String((err as { message?: string })?.message ?? err));
    }
  };

  const handleChooseProjectInstallFolder = async () => {
    try {
      const selected = await selectProjectDirectory();
      if (selected) setProjectInstallPath(selected);
    } catch (err) {
      toast.danger(String((err as { message?: string })?.message ?? err));
    }
  };

  const openProjectInstallDialog = (skill: ManagedSkill) => {
    setProjectInstallSkill(skill);
    setProjectInstallPath("");
    setProjectInstallRuntime((current) =>
      projectRuntimeChoices.includes(current) ? current : (projectRuntimeChoices[0] ?? "codex"),
    );
  };

  const customImportPending = importSkill.isPending && importSkill.variables?.skillId === "";
  const customProjectTargets =
    customProjectInstall && customProjectPath.trim()
      ? [{ runtime: customProjectRuntime, projectRoot: customProjectPath.trim() }]
      : undefined;

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
                <Modal.CloseTrigger />
                <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
                  <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                    {t("local.unmanaged")}
                  </Modal.Heading>
                  <p className="mt-1 text-[12px] text-[var(--fg-muted)]">{t("local.unmanaged.desc")}</p>
                </Modal.Header>

                <Modal.Body className="max-h-[66vh] overflow-y-auto px-5 py-4">
                  <div className="mb-4 rounded-lg border border-[var(--line)] bg-[var(--bg)] p-3">
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2 text-[13px] font-medium text-[var(--fg)]">
                          <FolderOpen size={15} className="text-[var(--fg-muted)]" />
                          {t("local.importCustom")}
                        </div>
                        <div className="mt-1 text-[11.5px] leading-[1.5] text-[var(--fg-muted)]">
                          {t("local.projectSkillHint")}
                        </div>
                      </div>
                      <Button size="sm" variant="outline" onPress={handleChooseSkillFolder}>
                        <FolderOpen size={13} />
                        {t("local.chooseFolder")}
                      </Button>
                    </div>
                    <div className="mt-3 flex items-center gap-2">
                      <input
                        value={customSkillPath}
                        onChange={(event) => setCustomSkillPath(event.target.value)}
                        placeholder={t("local.folderPathPlaceholder")}
                        className="min-w-0 flex-1 rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2 font-mono text-[12px] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                      />
                      <Button
                        size="sm"
                        variant="secondary"
                        isDisabled={!customSkillPath.trim() || (customProjectInstall && !customProjectPath.trim())}
                        isPending={customImportPending}
                        onPress={() =>
                          importSkill.mutate({
                            skillId: "",
                            sourcePath: customSkillPath.trim(),
                            projectTargets: customProjectTargets,
                          })
                        }
                      >
                        <Download size={12} />
                        {t("local.importSelectedFolder")}
                      </Button>
                    </div>
                    <div className="mt-3 rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2.5">
                      <div className="flex items-center justify-between gap-3">
                        <div className="min-w-0">
                          <div className="text-[12.5px] font-medium text-[var(--fg)]">
                            {t("local.importInstallToProject")}
                          </div>
                          <div className="mt-0.5 text-[11px] text-[var(--fg-muted)]">
                            {t("local.importInstallToProject.desc")}
                          </div>
                        </div>
                        <Switch isSelected={customProjectInstall} onChange={setCustomProjectInstall}>
                          <Switch.Control>
                            <Switch.Thumb />
                          </Switch.Control>
                        </Switch>
                      </div>
                      {customProjectInstall ? (
                        <div className="mt-3 space-y-2">
                          <div className="flex items-center gap-2">
                            <input
                              value={customProjectPath}
                              onChange={(event) => setCustomProjectPath(event.target.value)}
                              placeholder={t("install.projectRoot.placeholder")}
                              className="min-w-0 flex-1 rounded-md border border-[var(--line)] bg-[var(--bg)] px-3 py-2 font-mono text-[12px] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                            />
                            <Button size="sm" variant="outline" onPress={handleChooseProjectFolder}>
                              <FolderOpen size={13} />
                              {t("local.chooseFolder")}
                            </Button>
                          </div>
                          <div className="flex flex-wrap gap-1.5">
                            {projectRuntimeChoices.map((runtime) => (
                              <button
                                key={runtime}
                                type="button"
                                onClick={() => setCustomProjectRuntime(runtime)}
                                className={`rounded-md border px-2.5 py-1 text-[11.5px] transition-colors ${
                                  customProjectRuntime === runtime
                                    ? "border-[var(--brand)] bg-[var(--brand-soft)] text-[var(--brand-fg)]"
                                    : "border-[var(--line)] text-[var(--fg-secondary)] hover:bg-[var(--bg-soft)]"
                                }`}
                              >
                                {TOOL_LABELS[runtime] ?? runtime}
                              </button>
                            ))}
                          </div>
                        </div>
                      ) : null}
                    </div>
                  </div>

                  {unmanaged.isFetching && !groupedUnmanaged.length ? (
                    <div className="flex items-center justify-center py-8">
                      <Pill>{t("local.scanning")}</Pill>
                    </div>
                  ) : groupedUnmanaged.length ? (
                    <div className="space-y-2">
                      {groupedUnmanaged.map((group) => {
                        const open = openImportGroups[group.runtime] ?? true;
                        return (
                          <section
                            key={group.runtime}
                            className="overflow-hidden rounded-lg border border-[var(--line)] bg-[var(--bg-elevated)]"
                          >
                            <div className="flex items-center gap-2 border-b border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2">
                              <button
                                type="button"
                                className="flex min-w-0 flex-1 items-center gap-2 text-left"
                                onClick={() =>
                                  setOpenImportGroups((prev) => ({
                                    ...prev,
                                    [group.runtime]: !(prev[group.runtime] ?? true),
                                  }))
                                }
                              >
                                <ChevronDown
                                  size={14}
                                  className={cn("shrink-0 text-[var(--fg-muted)] transition-transform", !open && "-rotate-90")}
                                />
                                <div className="min-w-0">
                                  <div className="flex items-center gap-2">
                                    <span className="truncate text-[12.5px] font-semibold text-[var(--fg)]">
                                      {group.label}
                                    </span>
                                    <Pill>{group.items.length}</Pill>
                                  </div>
                                  <div className="mt-0.5 truncate font-mono text-[10.5px] text-[var(--fg-muted)]">
                                    {group.globalPath}
                                  </div>
                                </div>
                              </button>
                              <Button
                                size="sm"
                                variant="secondary"
                                onPress={() => {
                                  for (const skill of group.items) {
                                    importSkill.mutate({ skillId: skill.id, sourcePath: skill.sourcePath });
                                  }
                                }}
                              >
                                <Download size={12} />
                                {t("local.importAll")}
                              </Button>
                            </div>

                            {open ? (
                              <div>
                                {group.items.map((skill) => (
                                  <div
                                    key={`${group.runtime}-${skill.id}-${skill.sourcePath}`}
                                    className="flex items-center justify-between gap-3 border-b border-[var(--line)] px-3 py-3 last:border-b-0 hover:bg-[var(--bg-soft)]"
                                  >
                                    <div className="min-w-0 flex-1">
                                      <div className="flex items-center gap-2">
                                        <Package size={14} className="shrink-0 text-[var(--fg-muted)]" />
                                        <span className="truncate text-[13px] font-medium">{skill.name}</span>
                                      </div>
                                      <div className="mt-0.5 truncate font-mono text-[11px] text-[var(--fg-muted)]">
                                        {skill.sourcePath}
                                      </div>
                                    </div>
                                    <Button
                                      size="sm"
                                      variant="secondary"
                                      onPress={() => importSkill.mutate({ skillId: skill.id, sourcePath: skill.sourcePath })}
                                      isPending={
                                        importSkill.isPending &&
                                        importSkill.variables?.skillId === skill.id &&
                                        importSkill.variables?.sourcePath === skill.sourcePath
                                      }
                                    >
                                      <Download size={12} />
                                      {t("local.import")}
                                    </Button>
                                  </div>
                                ))}
                              </div>
                            ) : null}
                          </section>
                        );
                      })}
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

        <Modal
          isOpen={Boolean(projectInstallSkill)}
          onOpenChange={(open) => {
            if (!open) {
              setProjectInstallSkill(null);
              setProjectInstallPath("");
            }
          }}
        >
          <Modal.Backdrop>
            <Modal.Container size="md">
              <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
                <Modal.CloseTrigger />
                <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
                  <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                    {t("mySkills.projectAddTitle").replace("{name}", projectInstallSkill?.name ?? "")}
                  </Modal.Heading>
                  <p className="mt-1 text-[12px] text-[var(--fg-muted)]">{t("mySkills.projectAddDesc")}</p>
                </Modal.Header>

                <Modal.Body className="space-y-4 px-5 py-4">
                  <section>
                    <div className="mb-2 text-[12px] font-medium text-[var(--fg-secondary)]">
                      {t("install.projectRoot")}
                    </div>
                    <div className="flex items-center gap-2">
                      <input
                        value={projectInstallPath}
                        onChange={(event) => setProjectInstallPath(event.target.value)}
                        placeholder={t("install.projectRoot.placeholder")}
                        className="min-w-0 flex-1 rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2 font-mono text-[12px] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                      />
                      <Button size="sm" variant="outline" onPress={handleChooseProjectInstallFolder}>
                        <FolderOpen size={13} />
                        {t("local.chooseFolder")}
                      </Button>
                    </div>
                    <div className="mt-1.5 text-[11px] text-[var(--fg-muted)]">
                      {t("install.projectRoot.desc")}
                    </div>
                  </section>

                  <section>
                    <div className="mb-2 text-[12px] font-medium text-[var(--fg-secondary)]">
                      {t("install.projectRuntime")}
                    </div>
                    <div className="grid grid-cols-2 gap-2">
                      {projectRuntimeChoices.map((runtime) => {
                        const selected = projectInstallRuntime === runtime;
                        return (
                          <button
                            key={runtime}
                            type="button"
                            onClick={() => setProjectInstallRuntime(runtime)}
                            className={cn(
                              "rounded-md border px-3 py-2 text-left text-[12.5px] transition-colors",
                              selected
                                ? "border-[var(--brand)] bg-[var(--brand-soft)] text-[var(--brand-fg)]"
                                : "border-[var(--line)] bg-[var(--bg-elevated)] text-[var(--fg)] hover:bg-[var(--bg-soft)]",
                            )}
                          >
                            {TOOL_LABELS[runtime] ?? runtime}
                          </button>
                        );
                      })}
                    </div>
                  </section>
                </Modal.Body>

                <div className="flex justify-end gap-2 border-t border-[var(--line)] px-5 py-3">
                  <Button size="sm" variant="outline" onPress={() => setProjectInstallSkill(null)}>
                    {t("common.cancel")}
                  </Button>
                  <Button
                    size="sm"
                    isDisabled={!projectInstallSkill || !projectInstallPath.trim()}
                    isPending={addProjectDeployment.isPending}
                    onPress={() => {
                      if (!projectInstallSkill) return;
                      addProjectDeployment.mutate({
                        skillId: projectInstallSkill.id,
                        runtime: projectInstallRuntime,
                        projectRoot: projectInstallPath.trim(),
                      });
                    }}
                  >
                    <FolderPlus size={13} />
                    {t("mySkills.projectAddConfirm")}
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
                busyProjectDeploymentId={busyProjectDeploymentId}
                deletingProjectDeploymentId={
                  deleteProjectDeployment.isPending ? (deleteProjectDeployment.variables ?? null) : null
                }
                onToggleProjectDeployment={(deploymentId, enabled) =>
                  toggleProjectDeployment.mutate({ deploymentId, enabled })
                }
                onDeleteProjectDeployment={(deploymentId) => deleteProjectDeployment.mutate(deploymentId)}
                onAddProjectInstall={() => openProjectInstallDialog(skill)}
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
                pathOpeners={pathOpeners.data ?? []}
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

function projectDisplayName(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/$/, "");
  return normalized.split("/").filter(Boolean).pop() || path;
}

function sourceWorkspaceUrl(sourceWorkspace: string): string | null {
  const value = sourceWorkspace.trim();
  if (!value || value === "local" || !value.includes("/")) return null;
  return `https://github.com/${value}`;
}

function SourceLink({ sourceWorkspace, sourceBranch }: { sourceWorkspace: string; sourceBranch: string }) {
  const url = sourceWorkspaceUrl(sourceWorkspace);
  const label = `${sourceWorkspace}${sourceBranch ? `@${sourceBranch}` : ""}`;
  if (!url) {
    return <div className="mt-1 truncate font-mono text-[11px] text-[var(--fg-muted)]">{label}</div>;
  }
  return (
    <button
      type="button"
      className="mt-1 inline-flex max-w-full items-center gap-1 truncate font-mono text-[11px] text-[var(--fg-muted)] hover:text-[var(--brand)]"
      onClick={() => void openExternalUrl(url)}
    >
      <span className="truncate">{label}</span>
      <ExternalLink size={11} className="shrink-0" />
    </button>
  );
}

function OpenerIcon({
  opener,
  size = 16,
  variant = "small",
}: {
  opener: PathOpener;
  size?: number;
  variant?: "small" | "default" | "large";
}) {
  const iconUrl = opener.iconUrls?.[variant] ?? opener.iconUrl;
  if (iconUrl) {
    return (
      <img
        src={iconUrl}
        alt=""
        draggable={false}
        className="shrink-0 rounded-[4px]"
        style={{ width: size, height: size }}
      />
    );
  }
  return null;
}

function PathOpenButton({ path, openers }: { path: string; openers: PathOpener[] }) {
  const { t } = useLocale();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement | null>(null);
  const available = openers.length ? openers : [{ id: "default", label: t("mySkills.open"), appName: null }];
  const primary = available[0];

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  const openWith = (opener: PathOpener) => {
    setOpen(false);
    void openLocalPath(path, opener.id).catch((err) =>
      toast.danger(String((err as { message?: string })?.message ?? err)),
    );
  };

  return (
    <div ref={ref} className="relative flex shrink-0 items-center">
      <button
        type="button"
        className="inline-flex h-7 w-10 items-center justify-center rounded-l-md border border-[var(--line)] bg-[var(--bg)] text-[var(--fg-secondary)] hover:bg-[var(--bg-soft)] hover:text-[var(--fg)]"
        onClick={() => openWith(primary)}
        title={`${t("mySkills.openWith")}: ${primary.label}`}
        aria-label={`${t("mySkills.openWith")}: ${primary.label}`}
      >
        <OpenerIcon opener={primary} size={17} variant="small" />
      </button>
      <button
        type="button"
        className="inline-flex h-7 w-7 items-center justify-center rounded-r-md border-y border-r border-[var(--line)] bg-[var(--bg)] text-[var(--fg-muted)] hover:bg-[var(--bg-soft)] hover:text-[var(--fg)]"
        onClick={() => setOpen((value) => !value)}
        aria-label={t("mySkills.openWith")}
      >
        <ChevronDown size={13} />
      </button>
      {open ? (
        <div className="absolute right-0 top-8 z-20 min-w-[180px] overflow-hidden rounded-lg border border-[var(--line)] bg-[var(--bg-elevated)] p-1 shadow-lg">
          {available.map((opener) => (
            <button
              key={opener.id}
              type="button"
              className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-[12.5px] text-[var(--fg)] hover:bg-[var(--bg-soft)]"
              onClick={() => openWith(opener)}
            >
              <OpenerIcon opener={opener} size={18} variant="small" />
              {opener.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function ProjectDeploymentRow({
  deployment,
  pathOpeners,
  isToggling,
  isDeleting,
  onToggle,
  onDelete,
}: {
  deployment: ManagedSkillProjectDeployment;
  pathOpeners: PathOpener[];
  isToggling: boolean;
  isDeleting: boolean;
  onToggle: (enabled: boolean) => void;
  onDelete: () => void;
}) {
  const { t } = useLocale();
  const toggleLabel = deployment.enabled ? t("mySkills.projectDisable") : t("mySkills.projectEnable");

  return (
    <div
      className={cn(
        "flex items-center justify-between gap-3 rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-2.5 py-2",
        !deployment.enabled && "bg-[var(--bg)]",
      )}
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-[12.5px] font-medium text-[var(--fg)]">
            {projectDisplayName(deployment.projectRoot)}
          </span>
          <Pill mono>{TOOL_LABELS[deployment.runtime] ?? deployment.runtime}</Pill>
          {!deployment.enabled ? <Pill>{t("mySkills.projectPaused")}</Pill> : null}
          {deployment.enabled && deployment.status === "missing" ? (
            <Pill tone="danger">{t("mySkills.projectMissing")}</Pill>
          ) : deployment.enabled ? (
            <Pill tone="success">{t("mySkills.projectActive")}</Pill>
          ) : null}
        </div>
        <div className="mt-0.5 truncate font-mono text-[10.5px] text-[var(--fg-muted)]">
          {deployment.targetPath}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        <Tooltip delay={150}>
          <Switch
            isSelected={deployment.enabled}
            isDisabled={isToggling || isDeleting}
            onChange={onToggle}
            aria-label={toggleLabel}
          >
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
          </Switch>
          <Tooltip.Content>{toggleLabel}</Tooltip.Content>
        </Tooltip>
        <PathOpenButton
          path={deployment.status === "missing" ? deployment.projectRoot : deployment.targetPath}
          openers={pathOpeners}
        />
        <AlertDialog>
          <Button
            size="sm"
            variant="ghost"
            className="text-[var(--fg-muted)] hover:text-[var(--danger)]"
            isDisabled={isToggling || isDeleting}
            aria-label={t("mySkills.projectRemove")}
          >
            <Trash2 size={14} />
          </Button>
          <AlertDialog.Backdrop>
            <AlertDialog.Container size="sm">
              <AlertDialog.Dialog className="sm:max-w-[420px]">
                <AlertDialog.CloseTrigger />
                <AlertDialog.Header>
                  <AlertDialog.Icon status="danger" />
                  <AlertDialog.Heading>{t("mySkills.projectRemoveTitle")}</AlertDialog.Heading>
                </AlertDialog.Header>
                <AlertDialog.Body>
                  <div className="space-y-2 text-[13px] leading-[1.5] text-[var(--fg-secondary)]">
                    <p>{t("mySkills.projectRemoveDesc")}</p>
                    <div className="rounded-md border border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2">
                      <div className="truncate font-medium text-[var(--fg)]">
                        {projectDisplayName(deployment.projectRoot)}
                      </div>
                      <div className="mt-0.5 truncate font-mono text-[11px] text-[var(--fg-muted)]">
                        {deployment.targetPath}
                      </div>
                    </div>
                  </div>
                </AlertDialog.Body>
                <AlertDialog.Footer>
                  <Button slot="close" variant="outline">
                    {t("common.cancel")}
                  </Button>
                  <Button slot="close" variant="danger-soft" onPress={onDelete} isPending={isDeleting}>
                    {t("mySkills.projectRemoveConfirm")}
                  </Button>
                </AlertDialog.Footer>
              </AlertDialog.Dialog>
            </AlertDialog.Container>
          </AlertDialog.Backdrop>
        </AlertDialog>
      </div>
    </div>
  );
}

function SkillCard({
  skill,
  tools,
  busyRuntimeKey,
  onToggle,
  busyProjectDeploymentId,
  deletingProjectDeploymentId,
  onToggleProjectDeployment,
  onDeleteProjectDeployment,
  onAddProjectInstall,
  onRemove,
  removing,
  onRetry,
  retrying,
  onReview,
  reviewing,
  aiConfigured,
  onPush,
  pathOpeners,
}: {
  skill: ManagedSkill;
  tools: string[];
  busyRuntimeKey: string | null;
  onToggle: (runtime: string, currentlyEnabled: boolean) => void;
  busyProjectDeploymentId: number | null;
  deletingProjectDeploymentId: number | null;
  onToggleProjectDeployment: (deploymentId: number, enabled: boolean) => void;
  onDeleteProjectDeployment: (deploymentId: number) => void;
  onAddProjectInstall: () => void;
  onRemove: () => void;
  removing: boolean;
  onRetry: () => void;
  retrying: boolean;
  onReview: () => void;
  reviewing: boolean;
  aiConfigured: boolean;
  onPush: () => void;
  pathOpeners: PathOpener[];
}) {
  const { t } = useLocale();
  const [showFindings, setShowFindings] = useState(false);
  const [autoUpdate, setAutoUpdate] = useLocalStorage<boolean>(`my-skills:auto:${skill.id}`, true);

  const isDownloading = skill.installStatus === "downloading";
  const isError = skill.installStatus === "error";
  const hasReview = skill.reviewVerdict !== "";
  const projectDeployments = skill.projectDeployments ?? [];

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
            <SourceLink sourceWorkspace={skill.sourceWorkspace} sourceBranch={skill.sourceBranch} />
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
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
          <AlertDialog>
            <Button
              size="sm"
              variant="ghost"
              className="text-[var(--fg-muted)] hover:text-[var(--danger)]"
              isDisabled={removing}
              aria-label={t("mySkills.remove")}
            >
              <Trash2 size={14} />
            </Button>
            <AlertDialog.Backdrop>
              <AlertDialog.Container size="sm">
                <AlertDialog.Dialog className="sm:max-w-[420px]">
                  <AlertDialog.CloseTrigger />
                  <AlertDialog.Header>
                    <AlertDialog.Icon status="danger" />
                    <AlertDialog.Heading>{t("mySkills.removeTitle")}</AlertDialog.Heading>
                  </AlertDialog.Header>
                  <AlertDialog.Body>
                    <div className="space-y-2 text-[13px] leading-[1.5] text-[var(--fg-secondary)]">
                      <p>{t("mySkills.removeDesc")}</p>
                      <div className="rounded-md border border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2">
                        <div className="truncate font-medium text-[var(--fg)]">{skill.name}</div>
                        {skill.localPath ? (
                          <div className="mt-0.5 truncate font-mono text-[11px] text-[var(--fg-muted)]">
                            {skill.localPath}
                          </div>
                        ) : null}
                      </div>
                    </div>
                  </AlertDialog.Body>
                  <AlertDialog.Footer>
                    <Button slot="close" variant="outline">
                      {t("common.cancel")}
                    </Button>
                    <Button slot="close" variant="danger-soft" onPress={onRemove} isPending={removing}>
                      {t("mySkills.remove.confirm")}
                    </Button>
                  </AlertDialog.Footer>
                </AlertDialog.Dialog>
              </AlertDialog.Container>
            </AlertDialog.Backdrop>
          </AlertDialog>
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
            <Tooltip delay={150}>
              <Button
                size="sm"
                variant="outline"
                className="h-[34px] px-2.5 text-[var(--fg-secondary)] hover:text-[var(--brand)]"
                onPress={onAddProjectInstall}
                aria-label={t("mySkills.projectAdd")}
              >
                <FolderPlus size={14} />
              </Button>
              <Tooltip.Content>{t("mySkills.projectAdd")}</Tooltip.Content>
            </Tooltip>
          </div>

          {projectDeployments.length ? (
            <div className="mt-3 rounded-md border border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2.5">
              <div className="mb-2 flex items-center gap-2 text-[12px] font-medium text-[var(--fg-secondary)]">
                <FolderOpen size={13} className="text-[var(--fg-muted)]" />
                {t("mySkills.projects")}
                <Pill>{projectDeployments.length}</Pill>
              </div>
              <div className="space-y-1.5">
                {projectDeployments.map((deployment) => (
                  <ProjectDeploymentRow
                    key={deployment.id}
                    deployment={deployment}
                    pathOpeners={pathOpeners}
                    isToggling={busyProjectDeploymentId === deployment.id}
                    isDeleting={deletingProjectDeploymentId === deployment.id}
                    onToggle={(enabled) => onToggleProjectDeployment(deployment.id, enabled)}
                    onDelete={() => onDeleteProjectDeployment(deployment.id)}
                  />
                ))}
              </div>
            </div>
          ) : null}

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
