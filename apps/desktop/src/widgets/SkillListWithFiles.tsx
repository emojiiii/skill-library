import { ChevronDown, ChevronRight, Folder } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { SkillAsset } from "../lib/skill-library";
import { useLocalStorage } from "../hooks/useLocalStorage";
import { useLocale } from "../hooks/useLocale";
import { SkillCard } from "./SkillCard";
import { SkillFileTree } from "./SkillFileTree";

interface CategoryGroup {
  name: string;
  assets: SkillAsset[];
}

function groupByCategory(assets: SkillAsset[]): CategoryGroup[] {
  const groups = new Map<string, SkillAsset[]>();
  const ungrouped: SkillAsset[] = [];

  for (const asset of assets) {
    const parts = asset.path.split("/").filter(Boolean);
    if (parts.length >= 3) {
      const category = parts.slice(0, -1).join("/");
      if (!groups.has(category)) groups.set(category, []);
      groups.get(category)!.push(asset);
    } else if (parts.length === 2) {
      const category = parts[0];
      if (!groups.has(category)) groups.set(category, []);
      groups.get(category)!.push(asset);
    } else {
      ungrouped.push(asset);
    }
  }

  const result: CategoryGroup[] = [];
  if (ungrouped.length > 0) {
    result.push({ name: "", assets: ungrouped });
  }
  for (const [name, items] of groups) {
    result.push({ name, assets: items });
  }
  return result;
}

function CategorySection({
  group,
  selected,
  selectedFile,
  expandedSkills,
  workspace,
  canViewFiles,
  onSelectAsset,
  onToggleExpand,
  onSelectFile,
  defaultExpanded,
}: {
  group: CategoryGroup;
  selected: SkillAsset | null;
  selectedFile: string | null;
  expandedSkills: string[];
  workspace: string;
  canViewFiles: boolean;
  onSelectAsset: (asset: SkillAsset) => void;
  onToggleExpand: (id: string) => void;
  onSelectFile: (path: string | null) => void;
  defaultExpanded: boolean;
}) {
  const [expanded, setExpanded] = useLocalStorage<boolean>(
    `ws-ui:${workspace}:cat:${group.name}`,
    defaultExpanded,
  );

  if (!group.name) {
    return (
      <>
        {group.assets.map((asset) => (
          <SkillItem
            key={asset.manifest.id}
            asset={asset}
            selected={selected}
            selectedFile={selectedFile}
            isExpanded={expandedSkills.includes(asset.manifest.id)}
            workspace={workspace}
            canViewFiles={canViewFiles}
            onSelectAsset={onSelectAsset}
            onToggleExpand={onToggleExpand}
            onSelectFile={onSelectFile}
          />
        ))}
      </>
    );
  }

  const displayName = group.name.split("/").pop() ?? group.name;

  return (
    <div className="mb-1">
      <button
        type="button"
        className="skill-tree-group"
        onClick={() => setExpanded((v) => !v)}
      >
        <span className="skill-tree-group__chevron">
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </span>
        <Folder size={13} className="text-[var(--fg-muted)]" />
        <span className="skill-tree-group__name">{displayName}</span>
        <span className="skill-tree-group__count">{group.assets.length}</span>
      </button>
      {expanded ? (
        <div className="ml-2">
          {group.assets.map((asset) => (
            <SkillItem
              key={asset.manifest.id}
              asset={asset}
              selected={selected}
              selectedFile={selectedFile}
              isExpanded={expandedSkills.includes(asset.manifest.id)}
              workspace={workspace}
              canViewFiles={canViewFiles}
              onSelectAsset={onSelectAsset}
              onToggleExpand={onToggleExpand}
              onSelectFile={onSelectFile}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function SkillItem({
  asset,
  selected,
  selectedFile,
  isExpanded,
  workspace,
  canViewFiles,
  onSelectAsset,
  onToggleExpand,
  onSelectFile,
}: {
  asset: SkillAsset;
  selected: SkillAsset | null;
  selectedFile: string | null;
  isExpanded: boolean;
  workspace: string;
  canViewFiles: boolean;
  onSelectAsset: (asset: SkillAsset) => void;
  onToggleExpand: (id: string) => void;
  onSelectFile: (path: string | null) => void;
}) {
  const { t } = useLocale();
  const isSelected = selected?.manifest.id === asset.manifest.id;
  // Mount the file tree only after the first expand, then keep it mounted so
  // both expand and collapse animate (and the query fires only once needed).
  const [hasExpanded, setHasExpanded] = useState(isExpanded);
  useEffect(() => {
    if (isExpanded) setHasExpanded(true);
  }, [isExpanded]);

  return (
    <div>
      <SkillCard
        asset={asset}
        selected={isSelected}
        expanded={isExpanded}
        onSelect={() => {
          onSelectAsset(asset);
          onToggleExpand(asset.manifest.id);
        }}
      />
      <div className={`skill-item__files ${isExpanded ? "is-open" : ""}`}>
        <div className="skill-item__files-inner">
          {hasExpanded ? (
            <div className="ml-6 mt-0.5 mb-1.5 border-l-2 border-[var(--line)] pl-2">
              {canViewFiles && workspace ? (
                <SkillFileTree
                  workspace={workspace}
                  skillPath={asset.path}
                  selectedFile={selectedFile}
                  onSelectFile={(path) => {
                    // Also select this skill when clicking a file under it
                    if (!isSelected) onSelectAsset(asset);
                    onSelectFile(path);
                  }}
                />
              ) : (
                <div className="px-3 py-2 text-[11px] text-[var(--fg-muted)]">
                  {t("workspaces.loginToViewFiles")}
                </div>
              )}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

export function SkillListWithFiles({
  assets,
  selected,
  selectedFile,
  workspace,
  canViewFiles,
  onSelectAsset,
  onSelectFile,
}: {
  assets: SkillAsset[];
  selected: SkillAsset | null;
  selectedFile: string | null;
  workspace: string;
  canViewFiles: boolean;
  onSelectAsset: (asset: SkillAsset) => void;
  onSelectFile: (path: string | null) => void;
}) {
  const groups = useMemo(() => groupByCategory(assets), [assets]);
  const [expandedSkills, setExpandedSkills] = useLocalStorage<string[]>(
    `ws-ui:${workspace}:expandedSkills`,
    selected ? [selected.manifest.id] : [],
  );

  const handleToggleExpand = (id: string) => {
    setExpandedSkills((prev) => {
      if (prev.includes(id)) {
        return prev.filter((x) => x !== id);
      }
      return [...prev, id];
    });
  };

  if (groups.length === 1 && !groups[0].name) {
    return (
      <div className="skill-list">
        {assets.map((asset) => (
          <SkillItem
            key={asset.manifest.id}
            asset={asset}
            selected={selected}
            selectedFile={selectedFile}
            isExpanded={expandedSkills.includes(asset.manifest.id)}
            workspace={workspace}
            canViewFiles={canViewFiles}
            onSelectAsset={onSelectAsset}
            onToggleExpand={handleToggleExpand}
            onSelectFile={onSelectFile}
          />
        ))}
      </div>
    );
  }

  return (
    <div className="skill-list">
      {groups.map((group) => (
        <CategorySection
          key={group.name || "__ungrouped"}
          group={group}
          selected={selected}
          selectedFile={selectedFile}
          expandedSkills={expandedSkills}
          workspace={workspace}
          canViewFiles={canViewFiles}
          onSelectAsset={onSelectAsset}
          onToggleExpand={handleToggleExpand}
          onSelectFile={onSelectFile}
          defaultExpanded={groups.length <= 5}
        />
      ))}
    </div>
  );
}
