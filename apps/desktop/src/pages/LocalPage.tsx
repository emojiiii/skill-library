import { Button, Modal, Switch, Tooltip } from "@heroui/react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Download, GitPullRequestArrow, Package, RefreshCw, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import {
  dbDisableSkill,
  dbEnableSkill,
  dbImportSkill,
  dbListRuntimes,
  dbListSkills,
  dbScanUnmanaged,
  type LocalAgentEntry,
  type ManagedSkill,
  type SupportedRuntime,
  type UnmanagedSkillInfo,
} from "../lib/teamai";
import { Pill } from "../widgets/Pill";

export type RuntimeKind = string;

export function LocalPage({
  roots,
  pending,
  error,
  onRefresh,
  onToggleRuntime,
  onPush,
  toggleBusyId,
  workspaceCount,
}: {
  roots: unknown[];
  pending: boolean;
  error: string | null;
  onRefresh: () => void;
  onToggleRuntime: (entry: LocalAgentEntry, runtime: RuntimeKind, enable: boolean) => void;
  onPush: (entry: LocalAgentEntry) => void;
  toggleBusyId: string | null;
  workspaceCount: number;
}) {
  const queryClient = useQueryClient();
  const [showImport, setShowImport] = useState(false);

  // Fetch managed skills from SQLite
  const managedSkills = useQuery({
    queryKey: ["db-skills"],
    queryFn: dbListSkills,
    staleTime: 30 * 1000,
  });

  // Fetch supported runtimes
  const runtimes = useQuery({
    queryKey: ["db-runtimes"],
    queryFn: dbListRuntimes,
    staleTime: 5 * 60 * 1000,
  });

  // Fetch unmanaged skills (for import)
  const unmanaged = useQuery({
    queryKey: ["db-unmanaged"],
    queryFn: dbScanUnmanaged,
    staleTime: 30 * 1000,
    enabled: showImport,
  });

  // Enable/disable mutations
  const enableSkill = useMutation({
    mutationFn: (args: { skillId: string; runtime: string }) =>
      dbEnableSkill(args.skillId, args.runtime),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
  });

  const disableSkill = useMutation({
    mutationFn: (args: { skillId: string; runtime: string }) =>
      dbDisableSkill(args.skillId, args.runtime),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["db-skills"] }),
  });

  const importSkill = useMutation({
    mutationFn: (args: { skillId: string; sourcePath: string }) =>
      dbImportSkill(args.skillId, args.sourcePath),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["db-skills"] });
      queryClient.invalidateQueries({ queryKey: ["db-unmanaged"] });
    },
  });

  const skills = managedSkills.data ?? [];
  // Only show runtimes that exist on this machine
  const activeRuntimes = (runtimes.data ?? []).filter((r) => r.exists);
  // Show at most 5 runtime columns to avoid overflow
  const displayRuntimes = activeRuntimes.slice(0, 5);

  const totalSkills = skills.length;
  const totalEnabled = skills.reduce(
    (sum, s) => sum + s.targets.filter((t) => t.enabled).length,
    0,
  );

  // Group unmanaged skills by source runtime
  const groupedUnmanaged = useMemo(() => {
    const items = unmanaged.data ?? [];
    const groups = new Map<string, UnmanagedSkillInfo[]>();
    for (const skill of items) {
      for (const runtime of skill.foundIn) {
        const list = groups.get(runtime) ?? [];
        list.push(skill);
        groups.set(runtime, list);
      }
    }
    // Sort groups alphabetically
    return [...groups.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [unmanaged.data]);

  const { t } = useLocale();

  return (
    <section className="scroll-area min-h-0 flex-1">
      <div className="mx-auto max-w-[1200px] p-6">
        <div className="mb-5 grid grid-cols-3 gap-3">
          <Stat label={t("local.managed")} value={totalSkills} />
          <Stat label={t("local.activeIntegrations")} value={totalEnabled} />
          <Stat label={t("local.runtimesDetected")} value={activeRuntimes.length} />
        </div>

        {/* Import section */}
        <div className="mb-4 flex items-center gap-2">
          <Button
            size="sm"
            variant={showImport ? "secondary" : "outline"}
            onPress={() => setShowImport(true)}
          >
            <Download size={13} />
            {t("local.importFromIde")}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onPress={() => {
              managedSkills.refetch();
              onRefresh();
            }}
            isPending={managedSkills.isFetching}
          >
            <RefreshCw size={13} />
            {t("local.refresh")}
          </Button>
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
                  <p className="mt-1 text-[12px] text-[var(--fg-muted)]">
                    {t("local.unmanaged.desc")}
                  </p>
                </Modal.Header>

                <Modal.Body className="max-h-[60vh] overflow-y-auto px-5 py-4">
                  {unmanaged.isFetching && !groupedUnmanaged.length ? (
                    <div className="flex items-center justify-center py-8">
                      <Pill>{t("local.scanning")}</Pill>
                    </div>
                  ) : groupedUnmanaged.length ? (
                    <div className="rounded-md border border-[var(--line)] overflow-hidden">
                      {groupedUnmanaged.map(([runtime, skills]) => (
                        <div key={runtime}>
                          <div className="sticky top-0 z-10 flex items-center justify-between border-b border-[var(--line)] bg-[var(--bg-soft)] px-4 py-2">
                            <div className="flex items-center gap-2">
                              <span className="text-[11px] font-semibold uppercase tracking-wide text-[var(--fg-muted)]">
                                {runtime}
                              </span>
                              <span className="text-[11px] text-[var(--fg-muted)]">({skills.length})</span>
                            </div>
                            <Button
                              size="sm"
                              variant="secondary"
                              onPress={() => {
                                for (const skill of skills) {
                                  importSkill.mutate({ skillId: skill.id, sourcePath: skill.path });
                                }
                              }}
                            >
                              <Download size={12} />
                              {t("local.importAll")}
                            </Button>
                          </div>
                          {skills.map((skill) => (
                            <div
                              key={`${runtime}-${skill.id}`}
                              className="flex items-center justify-between gap-3 border-b border-[var(--line)] px-4 py-3 last:border-b-0 hover:bg-[var(--bg-soft)]"
                            >
                              <div className="min-w-0 flex-1">
                                <div className="flex items-center gap-2">
                                  <Package size={14} className="shrink-0 text-[var(--fg-muted)]" />
                                  <span className="truncate text-[13px] font-medium">{skill.name}</span>
                                </div>
                                <div className="mt-0.5 text-[11px] text-[var(--fg-muted)]">
                                  <span className="truncate font-mono">{skill.id}</span>
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

        {/* Managed skills table */}
        <div className="card overflow-hidden">
          <div className="card-header">
            <div>
              <div className="card-title">{t("local.managedSkills")}</div>
              <div className="card-subtitle">
                {t("local.managedSkills.desc")}
              </div>
            </div>
            {pending || managedSkills.isFetching ? <Pill>{t("local.refreshing")}</Pill> : null}
          </div>

          {error ? (
            <div className="border-b border-[var(--line)] bg-[var(--danger-soft)] px-4 py-2 text-[12px] text-[var(--danger)]">
              {error}
            </div>
          ) : null}

          {skills.length === 0 ? (
            <div className="empty-state">
              <div className="empty-state__title">{t("local.noManaged")}</div>
            </div>
          ) : (
            <div className="overflow-x-auto">
              {/* Header row */}
              <div
                className="grid items-center gap-2 border-b border-[var(--line)] bg-[var(--bg-soft)] px-4 py-2 text-[10.5px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]"
                style={{
                  gridTemplateColumns: `minmax(0,1fr) ${displayRuntimes.map(() => "72px").join(" ")} 80px`,
                }}
              >
                <span>{t("nav.skills")}</span>
                {displayRuntimes.map((r) => (
                  <span key={r.id} className="text-center truncate" title={r.label}>
                    {r.label.split(" ")[0]}
                  </span>
                ))}
                <span className="text-right">{t("local.actions")}</span>
              </div>

              {/* Skill rows */}
              {skills.map((skill) => (
                <SkillRow
                  key={skill.id}
                  skill={skill}
                  runtimes={displayRuntimes}
                  onEnable={(runtime) => enableSkill.mutate({ skillId: skill.id, runtime })}
                  onDisable={(runtime) => disableSkill.mutate({ skillId: skill.id, runtime })}
                  busyKey={
                    enableSkill.isPending
                      ? `${enableSkill.variables?.skillId}:${enableSkill.variables?.runtime}`
                      : disableSkill.isPending
                        ? `${disableSkill.variables?.skillId}:${disableSkill.variables?.runtime}`
                        : null
                  }
                  onPush={onPush}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function SkillRow({
  skill,
  runtimes,
  onEnable,
  onDisable,
  busyKey,
  onPush,
}: {
  skill: ManagedSkill;
  runtimes: SupportedRuntime[];
  onEnable: (runtime: string) => void;
  onDisable: (runtime: string) => void;
  busyKey: string | null;
  onPush: (entry: LocalAgentEntry) => void;
}) {
  const { t } = useLocale();
  return (
    <div
      className="grid items-center gap-2 border-b border-[var(--line)] px-4 py-3 last:border-b-0 hover:bg-[var(--bg-soft)]"
      style={{
        gridTemplateColumns: `minmax(0,1fr) ${runtimes.map(() => "72px").join(" ")} 80px`,
      }}
    >
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <span className="truncate text-[13px] font-medium">{skill.name}</span>
          {skill.isModified ? (
            <Pill tone="warning">{t("local.modified")}</Pill>
          ) : null}
        </div>
        <div className="mt-0.5 flex items-center gap-2 text-[11px] text-[var(--fg-muted)]">
          <span className="truncate font-mono">{skill.id}</span>
          {skill.version ? <Pill mono>v{skill.version}</Pill> : null}
          <Pill>{skill.linkMode}</Pill>
          {skill.sourceWorkspace ? (
            <span className="truncate">← {skill.sourceWorkspace}{skill.sourceBranch ? `@${skill.sourceBranch}` : ""}</span>
          ) : null}
        </div>
        {skill.description ? (
          <div className="mt-1 line-clamp-1 text-[12px] text-[var(--fg-secondary)]">
            {skill.description}
          </div>
        ) : null}
      </div>

      {runtimes.map((runtime) => {
        const target = skill.targets.find((t) => t.runtime === runtime.id);
        const enabled = target?.enabled ?? false;
        const isBusy = busyKey === `${skill.id}:${runtime.id}`;
        return (
          <div key={runtime.id} className="flex items-center justify-center">
            <Switch
              isSelected={enabled}
              isDisabled={isBusy}
              onChange={() => {
                if (enabled) {
                  onDisable(runtime.id);
                } else {
                  onEnable(runtime.id);
                }
              }}
            >
              <Switch.Control>
                <Switch.Thumb />
              </Switch.Control>
            </Switch>
          </div>
        );
      })}

      <div className="flex items-center justify-end gap-1">
        <Tooltip delay={150}>
          <Button
            size="sm"
            variant="outline"
            onPress={() =>
              onPush({
                id: skill.id,
                name: skill.name,
                path: skill.localPath,
                hasManifest: true,
                hasSkillMd: true,
                managed: true,
                version: skill.version || null,
                description: skill.description || null,
              })
            }
          >
            <GitPullRequestArrow size={12} />
          </Button>
          <Tooltip.Content>{t("local.pushToWorkspace")}</Tooltip.Content>
        </Tooltip>
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="card px-4 py-3">
      <div className="text-[10.5px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">{label}</div>
      <div className="mt-1 text-[24px] font-semibold tracking-tight tabular-nums">{value}</div>
    </div>
  );
}
