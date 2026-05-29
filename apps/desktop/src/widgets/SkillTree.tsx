import { ChevronDown, ChevronRight, Folder } from "lucide-react";
import { useState } from "react";
import type { SkillAsset } from "../lib/teamai";
import { SkillCard } from "./SkillCard";

interface TreeNode {
  name: string;
  fullPath: string;
  skills: SkillAsset[];
  children: TreeNode[];
}

function buildTree(assets: SkillAsset[]): TreeNode {
  const root: TreeNode = { name: "", fullPath: "", skills: [], children: [] };

  for (const asset of assets) {
    const parts = asset.path.split("/").filter(Boolean);
    // The last segment is the skill folder itself; parent segments are the group
    if (parts.length <= 1) {
      // Top-level skill (no parent directory)
      root.skills.push(asset);
      continue;
    }

    // Group by all segments except the last one
    const groupParts = parts.slice(0, -1);
    let current = root;
    for (let i = 0; i < groupParts.length; i++) {
      const segment = groupParts[i];
      let child = current.children.find((c) => c.name === segment);
      if (!child) {
        child = {
          name: segment,
          fullPath: groupParts.slice(0, i + 1).join("/"),
          skills: [],
          children: [],
        };
        current.children.push(child);
      }
      current = child;
    }
    current.skills.push(asset);
  }

  return root;
}

/**
 * Collapse single-chain groups. If a node has exactly one child and no skills,
 * merge it with the child (e.g. "skills > dota2-arcade" → "skills/dota2-arcade").
 */
function collapseChain(node: TreeNode): TreeNode {
  let current = node;
  const nameParts: string[] = [];

  while (current.children.length === 1 && current.skills.length === 0) {
    const child = current.children[0];
    nameParts.push(child.name);
    current = child;
  }

  if (nameParts.length === 0) return node;

  return {
    name: nameParts.join("/"),
    fullPath: current.fullPath,
    skills: current.skills,
    children: current.children.map(collapseChain),
  };
}

function TreeGroup({
  node,
  selected,
  onSelect,
  depth,
  defaultExpanded,
}: {
  node: TreeNode;
  selected: SkillAsset | null;
  onSelect: (asset: SkillAsset) => void;
  depth: number;
  defaultExpanded: boolean;
}) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const totalCount = countSkills(node);

  return (
    <div>
      <button
        type="button"
        className="skill-tree-group"
        style={{ paddingLeft: `${depth * 12 + 8}px` }}
        onClick={() => setExpanded((v) => !v)}
      >
        <span className="skill-tree-group__chevron">
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </span>
        <Folder size={13} className="text-[var(--fg-muted)]" />
        <span className="skill-tree-group__name">{node.name}</span>
        <span className="skill-tree-group__count">{totalCount}</span>
      </button>

      {expanded ? (
        <div>
          {node.children.map((child) => (
            <TreeGroup
              key={child.fullPath}
              node={child}
              selected={selected}
              onSelect={onSelect}
              depth={depth + 1}
              defaultExpanded={defaultExpanded}
            />
          ))}
          {node.skills.map((asset) => (
            <SkillCard
              key={asset.manifest.id}
              asset={asset}
              selected={selected?.manifest.id === asset.manifest.id}
              onSelect={() => onSelect(asset)}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function countSkills(node: TreeNode): number {
  return node.skills.length + node.children.reduce((sum, c) => sum + countSkills(c), 0);
}

export function SkillTree({
  assets,
  selected,
  onSelect,
}: {
  assets: SkillAsset[];
  selected: SkillAsset | null;
  onSelect: (asset: SkillAsset) => void;
}) {
  const tree = buildTree(assets);

  const hasGroups = tree.children.length > 0;

  if (!hasGroups) {
    // No grouping needed — flat list
    return (
      <div className="skill-list">
        {assets.map((asset) => (
          <SkillCard
            key={asset.manifest.id}
            asset={asset}
            selected={selected?.manifest.id === asset.manifest.id}
            onSelect={() => onSelect(asset)}
          />
        ))}
      </div>
    );
  }

  // Collapse single-chain groups (e.g. skills > dota2-arcade → skills/dota2-arcade)
  const collapsedChildren = tree.children.map(collapseChain);

  // If after collapsing there's only one group with all skills and no root skills,
  // skip the group header entirely and just show a flat list
  if (collapsedChildren.length === 1 && tree.skills.length === 0 && collapsedChildren[0].children.length === 0) {
    return (
      <div className="skill-list">
        {collapsedChildren[0].skills.map((asset) => (
          <SkillCard
            key={asset.manifest.id}
            asset={asset}
            selected={selected?.manifest.id === asset.manifest.id}
            onSelect={() => onSelect(asset)}
          />
        ))}
      </div>
    );
  }

  return (
    <div className="skill-tree">
      {/* Root-level skills (no group) */}
      {tree.skills.map((asset) => (
        <SkillCard
          key={asset.manifest.id}
          asset={asset}
          selected={selected?.manifest.id === asset.manifest.id}
          onSelect={() => onSelect(asset)}
        />
      ))}

      {/* Grouped skills */}
      {collapsedChildren.map((child) => (
        <TreeGroup
          key={child.fullPath}
          node={child}
          selected={selected}
          onSelect={onSelect}
          depth={0}
          defaultExpanded={collapsedChildren.length <= 5}
        />
      ))}
    </div>
  );
}
