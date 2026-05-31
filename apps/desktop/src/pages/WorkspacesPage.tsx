import { useQuery } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, GitBranch, PanelLeftClose, PanelRightOpen, Search } from "lucide-react";
import { type ReactNode, useRef, useState } from "react";
import { Panel, PanelGroup, PanelResizeHandle, type ImperativePanelHandle } from "react-resizable-panels";
import { useLocale } from "../hooks/useLocale";
import type { SkillAsset, SkillVersion, Workspace, WorkspaceDetail } from "../lib/teamai";
import { listWorkspaceBranches } from "../lib/teamai";
import { SkillListWithFiles } from "../widgets/SkillListWithFiles";
import { Pill } from "../widgets/Pill";

export function WorkspacesPage({
  filteredAssets,
  selected,
  onSelectAsset,
  onSelectRef,
  selectedFile,
  onSelectFile,
  query,
  setQuery,
  workspaceMeta,
  workspaceDetail,
  workspaceRef,
  canViewFiles,
  scanPending,
  isRefreshing,
  detailPanel,
  versions,
  selectedBranch,
  onSelectBranch,
}: {
  filteredAssets: SkillAsset[];
  selected: SkillAsset | null;
  onSelectAsset: (asset: SkillAsset) => void;
  onSelectRef: (ref: string | undefined) => void;
  selectedFile: string | null;
  onSelectFile: (path: string | null) => void;
  query: string;
  setQuery: (value: string) => void;
  workspaceMeta: Workspace | null;
  workspaceDetail: WorkspaceDetail | null;
  workspaceRef: string;
  canViewFiles: boolean;
  scanPending: boolean;
  isRefreshing?: boolean;
  detailPanel: ReactNode;
  versions: SkillVersion[];
  selectedBranch: string | undefined;
  onSelectBranch: (branch: string | undefined) => void;
}) {
  const { t } = useLocale();
  const branches = useQuery({
    queryKey: ["workspace-branches", workspaceRef],
    queryFn: () => listWorkspaceBranches(workspaceRef),
    enabled: Boolean(workspaceMeta && workspaceRef),
    staleTime: 5 * 60 * 1000,
  });

  const leftPanelRef = useRef<ImperativePanelHandle>(null);
  const rightPanelRef = useRef<ImperativePanelHandle>(null);
  const [leftCollapsed, setLeftCollapsed] = useState(false);
  const [rightCollapsed, setRightCollapsed] = useState(false);

  return (
    <PanelGroup
      direction="horizontal"
      autoSaveId="workspace-panels"
      className="h-full min-h-0"
    >
      {/* Left panel: skill list */}
      <Panel
        ref={leftPanelRef}
        defaultSize={42}
        minSize={0}
        collapsible
        collapsedSize={0}
        order={1}
        onCollapse={() => setLeftCollapsed(true)}
        onExpand={() => setLeftCollapsed(false)}
      >
        <section className="flex h-full min-w-0 min-h-0 flex-col">
          <div className="flex items-center justify-between gap-3 border-b border-[var(--line)] bg-[var(--bg-elevated)] px-5 py-3">
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13.5px] font-semibold tracking-tight text-[var(--fg)]">
                {workspaceMeta ? workspaceMeta.full_name : t("workspaces.localWorkspace")}
              </div>
              <div className="mt-0.5 flex items-center gap-1.5">
                {workspaceMeta ? (
                  <>
                    <Pill>{workspaceMeta.visibility}</Pill>
                    <Pill tone="success">{workspaceMeta.permission}</Pill>
                    {/* Branch selector inline */}
                    <div className="flex items-center gap-1 ml-1">
                      <GitBranch size={11} className="text-[var(--fg-muted)]" />
                      <select
                        value={selectedBranch ?? workspaceMeta.default_branch}
                        onChange={(e) => {
                          const val = e.target.value;
                          onSelectBranch(val === workspaceMeta.default_branch ? undefined : val);
                        }}
                        className="appearance-none border-0 bg-transparent text-[11px] font-mono text-[var(--fg-muted)] outline-none cursor-pointer pr-3"
                        style={{ backgroundImage: "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 24 24' fill='none' stroke='%23888' stroke-width='2'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E\")", backgroundRepeat: "no-repeat", backgroundPosition: "right 0 center" }}
                      >
                        {(branches.data ?? [{ name: workspaceMeta.default_branch, isDefault: true }]).map((b) => (
                          <option key={b.name} value={b.name}>{b.name}</option>
                        ))}
                      </select>
                    </div>
                  </>
                ) : (
                  <Pill>{t("workspaces.localBundles")}</Pill>
                )}
              </div>
            </div>
            {/* Panel toggle buttons */}
            <div className="flex items-center gap-1">
              {rightCollapsed && (
                <button
                  type="button"
                  onClick={() => rightPanelRef.current?.expand()}
                  className="grid size-6 place-items-center rounded text-[var(--fg-muted)] hover:text-[var(--fg)] hover:bg-[var(--bg-soft)] transition-colors"
                  aria-label="Expand right panel"
                >
                  <PanelRightOpen size={14} />
                </button>
              )}
              <button
                type="button"
                onClick={() => leftPanelRef.current?.collapse()}
                className="grid size-6 place-items-center rounded text-[var(--fg-muted)] hover:text-[var(--fg)] hover:bg-[var(--bg-soft)] transition-colors"
                aria-label="Collapse left panel"
              >
                <PanelLeftClose size={14} />
              </button>
            </div>
          </div>

          <div className="px-5 pt-3 pb-2">
            <div className="relative">
              <Search size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--fg-muted)]" />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("workspaces.filterSkills")}
                className="w-full rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] py-2 pl-8 pr-3 text-[13px] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
              />
            </div>
            <div className="mt-2 flex items-center justify-between text-[11px] text-[var(--fg-muted)]">
              <span>
                {scanPending && !filteredAssets.length ? t("workspaces.scanning") : `${filteredAssets.length} ${filteredAssets.length === 1 ? t("workspaces.skillCount") : t("workspaces.skillCountPlural")}`}
              </span>
              {isRefreshing && filteredAssets.length > 0 ? (
                <span className="text-[var(--brand)]">{t("workspaces.syncing")}</span>
              ) : null}
            </div>
            {scanPending && !filteredAssets.length ? (
              <div className="scan-progress">
                <div className="scan-progress__bar" />
              </div>
            ) : null}
          </div>

          <div className="scroll-area px-5 pb-5 pt-1">
            {scanPending && !filteredAssets.length ? (
              <div className="skill-list">
                {Array.from({ length: 5 }).map((_, i) => (
                  <div key={i} className="skill-skeleton">
                    <div className="skill-skeleton__dot" />
                    <div className="skill-skeleton__lines">
                      <div className="skill-skeleton__line skill-skeleton__line--short" />
                      <div className="skill-skeleton__line skill-skeleton__line--long" />
                    </div>
                  </div>
                ))}
              </div>
            ) : filteredAssets.length ? (
              <SkillListWithFiles
                assets={filteredAssets}
                selected={selected}
                selectedFile={selectedFile}
                workspace={workspaceRef}
                canViewFiles={canViewFiles}
                onSelectAsset={(asset) => {
                  onSelectAsset(asset);
                  onSelectRef(undefined);
                  onSelectFile(null);
                }}
                onSelectFile={onSelectFile}
              />
            ) : (
              <div className="empty-state border border-dashed border-[var(--line)] rounded-md">
                <div className="empty-state__title">{t("workspaces.noSkills")}</div>
                <div>{t("workspaces.noSkills.desc")}</div>
              </div>
            )}
          </div>
        </section>
      </Panel>

      {/* Resize handle */}
      <PanelResizeHandle className="group relative flex w-[9px] shrink-0 cursor-col-resize items-center justify-center">
        {/* Thin vertical line */}
        <div className="h-full w-px bg-[var(--line)] group-hover:bg-[var(--brand)]/60 group-data-[resize-handle-active]:bg-[var(--brand)] transition-colors" />
        {/* Single toggle button */}
        <button
          type="button"
          className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-10 grid size-6 place-items-center rounded-full border border-[var(--line)] bg-[var(--bg-elevated)] text-[var(--fg-muted)] shadow-sm opacity-0 group-hover:opacity-100 hover:!border-[var(--brand)] hover:!text-[var(--brand)] hover:shadow transition-all"
          onClick={(e) => {
            e.stopPropagation();
            if (leftCollapsed) {
              leftPanelRef.current?.expand();
            } else {
              leftPanelRef.current?.collapse();
            }
          }}
          aria-label={leftCollapsed ? "Expand left panel" : "Collapse left panel"}
        >
          {leftCollapsed ? <ChevronRight size={13} strokeWidth={2} /> : <ChevronLeft size={13} strokeWidth={2} />}
        </button>
      </PanelResizeHandle>

      {/* Right panel: detail */}
      <Panel
        ref={rightPanelRef}
        defaultSize={58}
        minSize={0}
        collapsible
        collapsedSize={0}
        order={2}
        onCollapse={() => setRightCollapsed(true)}
        onExpand={() => setRightCollapsed(false)}
      >
        <aside className="flex h-full min-w-0 min-h-0 flex-col bg-[var(--bg-soft)]">
          {selected ? (
            detailPanel
          ) : (
            <div className="empty-state mx-auto my-auto max-w-md">
              <div className="empty-state__title">{t("workspaces.pickSkill")}</div>
              <div>
                {t("workspaces.pickSkill.desc")}
              </div>
            </div>
          )}
        </aside>
      </Panel>
    </PanelGroup>
  );
}
