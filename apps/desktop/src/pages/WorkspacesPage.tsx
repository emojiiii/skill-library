import { useQuery } from "@tanstack/react-query";
import { GitBranch, Search, ShieldCheck } from "lucide-react";
import { Modal } from "@heroui/react";
import type { ReactNode } from "react";
import { useLocale } from "../hooks/useLocale";
import type { SkillAsset, SkillVersion, Workspace, WorkspaceDetail } from "../lib/teamai";
import { listWorkspaceBranches } from "../lib/teamai";
import type { ReviewVerdictMap } from "../lib/review";
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
  detailOpen,
  onDetailOpenChange,
  versions,
  selectedBranch,
  onSelectBranch,
  reviewVerdicts,
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
  detailOpen: boolean;
  onDetailOpenChange: (open: boolean) => void;
  versions: SkillVersion[];
  selectedBranch: string | undefined;
  onSelectBranch: (branch: string | undefined) => void;
  reviewVerdicts?: ReviewVerdictMap;
}) {
  const { t } = useLocale();
  const branches = useQuery({
    queryKey: ["workspace-branches", workspaceRef],
    queryFn: () => listWorkspaceBranches(workspaceRef),
    enabled: Boolean(workspaceMeta && workspaceRef),
    staleTime: 5 * 60 * 1000,
  });

  return (
    <div className="flex h-full min-h-0">
      <section className="flex min-w-0 flex-1 flex-col">
        {/* Header: workspace info + search */}
        <div className="border-b border-[var(--line)] bg-[var(--bg-elevated)] px-6 py-4">
          <div className="mx-auto max-w-4xl">
            {/* Workspace name + branch */}
            <div className="flex items-center gap-3 mb-3">
              <div className="min-w-0 flex-1">
                <div className="truncate text-[14px] font-semibold tracking-tight text-[var(--fg)]">
                  {workspaceMeta ? workspaceMeta.full_name : t("workspaces.localWorkspace")}
                </div>
                <div className="mt-0.5 flex items-center gap-1.5">
                  {workspaceMeta ? (
                    <>
                      <Pill>{workspaceMeta.visibility}</Pill>
                      <Pill tone="success">{workspaceMeta.permission}</Pill>
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
            </div>

            {/* Search */}
            <div className="relative">
              <Search size={15} className="absolute left-3.5 top-1/2 -translate-y-1/2 text-[var(--fg-muted)]" />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("workspaces.filterSkills")}
                className="w-full rounded-lg border border-[var(--line)] bg-[var(--bg)] py-2.5 pl-10 pr-3 text-[14px] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
              />
            </div>
          </div>
        </div>

        {/* Grid content */}
        <div className="scroll-area flex-1 px-6 py-5">
          <div className="mx-auto max-w-4xl">
            <div className="mb-3 flex items-center justify-between">
              <span className="text-[12px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                {scanPending && !filteredAssets.length
                  ? t("workspaces.scanning")
                  : `${filteredAssets.length} ${filteredAssets.length === 1 ? t("workspaces.skillCount") : t("workspaces.skillCountPlural")}`}
              </span>
              {isRefreshing && filteredAssets.length > 0 ? (
                <span className="text-[11.5px] text-[var(--brand)]">{t("workspaces.syncing")}</span>
              ) : null}
            </div>

            {scanPending && !filteredAssets.length ? (
              <div className="scan-progress mb-4">
                <div className="scan-progress__bar" />
              </div>
            ) : null}

            {scanPending && !filteredAssets.length ? (
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
                {Array.from({ length: 6 }).map((_, i) => (
                  <div key={i} className="rounded-[12px] border border-[var(--line)] bg-[var(--bg-elevated)] p-4">
                    <div className="h-4 w-2/3 rounded bg-[var(--bg-soft)] animate-pulse" />
                    <div className="mt-2 h-3 w-full rounded bg-[var(--bg-soft)] animate-pulse" />
                    <div className="mt-1.5 h-3 w-1/2 rounded bg-[var(--bg-soft)] animate-pulse" />
                  </div>
                ))}
              </div>
            ) : filteredAssets.length ? (
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
                {filteredAssets.map((asset) => (
                  <SkillCard
                    key={asset.manifest.id}
                    asset={asset}
                    verdict={reviewVerdicts?.[asset.manifest.id]}
                    onSelect={() => {
                      onSelectAsset(asset);
                      onSelectRef(undefined);
                      onSelectFile(null);
                      onDetailOpenChange(true);
                    }}
                  />
                ))}
              </div>
            ) : (
              <div className="empty-state rounded-md border border-dashed border-[var(--line)]">
                <div className="empty-state__title">{t("workspaces.noSkills")}</div>
                <div>{t("workspaces.noSkills.desc")}</div>
              </div>
            )}
          </div>
        </div>
      </section>

      {/* Detail modal */}
      <Modal isOpen={detailOpen} onOpenChange={onDetailOpenChange}>
        <Modal.Backdrop>
          <Modal.Container>
            {/* Dimensions go through inline style: HeroUI's size variants apply a
                custom `.modal__dialog--*` class (= `max-w-*`) that tailwind-merge
                can't override from className, so a `w-[...]` utility gets capped.
                Inline style wins on specificity. */}
            <Modal.Dialog
              className="flex flex-col rounded-[14px] bg-[var(--bg-elevated)] outline-none"
              style={{ width: "min(1040px, 94vw)", maxWidth: "min(1040px, 94vw)", height: "min(760px, 88vh)" }}
            >
              {detailPanel}
            </Modal.Dialog>
          </Modal.Container>
        </Modal.Backdrop>
      </Modal>
    </div>
  );
}

/** Card for a single skill in the grid */
function SkillCard({
  asset,
  onSelect,
  verdict,
}: {
  asset: SkillAsset;
  onSelect: () => void;
  verdict?: string;
}) {
  const { t } = useLocale();
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
      }}
      className="group relative flex w-full cursor-pointer flex-col gap-2 rounded-[12px] border border-[var(--line)] bg-[var(--bg-elevated)] p-4 text-left transition-colors hover:border-[var(--brand)]/50 hover:bg-[var(--bg-soft)] focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-soft)]"
    >
      {/* Reviewed-safe badge — tilted green shield in the top-left corner */}
      {verdict === "safe" ? (
        <span
          className="absolute -left-1.5 -top-1.5 z-10 flex size-6 items-center justify-center rounded-full bg-[var(--success)] text-white shadow-sm"
          style={{ transform: "rotate(-12deg)" }}
          title={t("risk.safeBadge")}
        >
          <ShieldCheck size={13} strokeWidth={2.5} />
        </span>
      ) : null}
      <div className="flex items-start justify-between gap-2">
        <span className="truncate text-[14px] font-semibold tracking-tight text-[var(--fg)]">
          {asset.manifest.name}
        </span>
        <span className="shrink-0 rounded bg-[var(--bg-soft)] px-1.5 py-0.5 text-[10.5px] font-mono text-[var(--fg-muted)]">
          v{asset.manifest.version}
        </span>
      </div>
      {asset.manifest.description ? (
        <p className="line-clamp-2 text-[12px] leading-[1.5] text-[var(--fg-muted)]">
          {asset.manifest.description}
        </p>
      ) : (
        <p className="text-[12px] text-[var(--fg-muted)] opacity-50">{asset.path}</p>
      )}
      {asset.manifest.tags.length > 0 ? (
        <div className="mt-auto flex flex-wrap gap-1 pt-1">
          {asset.manifest.tags.slice(0, 3).map((tag) => (
            <span key={tag} className="rounded-full bg-[var(--bg-soft)] px-2 py-0.5 text-[10.5px] text-[var(--fg-muted)]">
              {tag}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}
