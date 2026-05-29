import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, File, FileText, Folder } from "lucide-react";
import { useMemo } from "react";
import { listSkillFiles, type SkillFileEntry } from "../lib/teamai";
import { getFileTreeFromCache, putFileTreeInCache } from "../lib/workspaceCache";
import { useLocalStorage } from "../hooks/useLocalStorage";

interface FileNode {
  name: string;
  fullPath: string;
  relativePath: string;
  kind: "file" | "directory";
  children: FileNode[];
  size?: number | null;
}

function buildFileTree(entries: SkillFileEntry[]): FileNode[] {
  const root: FileNode[] = [];

  // Sort: directories first, then alphabetical
  const sorted = [...entries].sort((a, b) => {
    if (a.kind !== b.kind) return a.kind === "directory" ? -1 : 1;
    return a.relativePath.localeCompare(b.relativePath);
  });

  for (const entry of sorted) {
    const parts = entry.relativePath.split("/").filter(Boolean);
    if (parts.length === 0) continue;

    let current = root;
    for (let i = 0; i < parts.length - 1; i++) {
      const dirName = parts[i];
      let dir = current.find((n) => n.name === dirName && n.kind === "directory");
      if (!dir) {
        dir = {
          name: dirName,
          fullPath: entry.path.split("/").slice(0, entry.path.split("/").indexOf(dirName) + 1).join("/"),
          relativePath: parts.slice(0, i + 1).join("/"),
          kind: "directory",
          children: [],
          size: null,
        };
        current.push(dir);
      }
      current = dir.children;
    }

    const lastName = parts[parts.length - 1];
    if (entry.kind === "directory") {
      if (!current.find((n) => n.name === lastName && n.kind === "directory")) {
        current.push({
          name: lastName,
          fullPath: entry.path,
          relativePath: entry.relativePath,
          kind: "directory",
          children: [],
          size: entry.size,
        });
      }
    } else {
      current.push({
        name: lastName,
        fullPath: entry.path,
        relativePath: entry.relativePath,
        kind: "file",
        children: [],
        size: entry.size,
      });
    }
  }

  return root;
}

function fileIcon(name: string) {
  const ext = name.split(".").pop()?.toLowerCase();
  if (name === "SKILL.md" || name === "manifest.yaml" || name === "manifest.yml") {
    return <FileText size={13} className="text-[var(--brand)]" />;
  }
  if (ext === "md") return <FileText size={13} className="text-[var(--fg-muted)]" />;
  return <File size={13} className="text-[var(--fg-muted)]" />;
}

function FileTreeNode({
  node,
  depth,
  selectedFile,
  onSelectFile,
  expandedDirs,
  onToggleDir,
}: {
  node: FileNode;
  depth: number;
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
  expandedDirs: Record<string, boolean>;
  onToggleDir: (dir: string) => void;
}) {
  const dirKey = node.fullPath;
  // Default: expand depth < 1, otherwise collapsed. Persisted state overrides.
  const expanded = dirKey in expandedDirs ? expandedDirs[dirKey] : depth < 1;

  if (node.kind === "directory") {
    return (
      <div>
        <button
          type="button"
          className="skill-file-tree__dir"
          style={{ paddingLeft: `${depth * 14 + 8}px` }}
          onClick={() => onToggleDir(dirKey)}
        >
          <span className="skill-file-tree__chevron">
            {expanded ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
          </span>
          <Folder size={13} className="text-[var(--fg-muted)]" />
          <span className="skill-file-tree__name">{node.name}</span>
        </button>
        {expanded ? (
          <div>
            {node.children.map((child) => (
              <FileTreeNode
                key={child.relativePath}
                node={child}
                depth={depth + 1}
                selectedFile={selectedFile}
                onSelectFile={onSelectFile}
                expandedDirs={expandedDirs}
                onToggleDir={onToggleDir}
              />
            ))}
          </div>
        ) : null}
      </div>
    );
  }

  const isSelected = selectedFile === node.fullPath;
  return (
    <button
      type="button"
      className={`skill-file-tree__file ${isSelected ? "is-selected" : ""}`}
      style={{ paddingLeft: `${depth * 14 + 8}px` }}
      onClick={() => onSelectFile(node.fullPath)}
    >
      {fileIcon(node.name)}
      <span className="skill-file-tree__name">{node.name}</span>
      {node.size != null ? (
        <span className="skill-file-tree__size">{formatSize(node.size)}</span>
      ) : null}
    </button>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}K`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}M`;
}

export function SkillFileTree({
  workspace,
  skillPath,
  refName,
  selectedFile,
  onSelectFile,
}: {
  workspace: string;
  skillPath: string;
  refName?: string;
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
}) {
  const [expandedDirs, setExpandedDirs] = useLocalStorage<Record<string, boolean>>(
    `ws-ui:${workspace}:${skillPath}:dirs`,
    {},
  );

  const handleToggleDir = (dir: string) => {
    setExpandedDirs((prev) => {
      const current = dir in prev ? prev[dir] : true; // default expanded for depth<1 handled in node
      return { ...prev, [dir]: !current };
    });
  };

  const { data: files, isLoading } = useQuery({
    queryKey: ["skill-files", workspace, skillPath, refName],
    queryFn: async () => {
      // Check IndexedDB cache first
      const cached = await getFileTreeFromCache(workspace, skillPath, refName);
      if (cached) return cached.files as SkillFileEntry[];
      // Cache miss — fetch from API
      const result = await listSkillFiles({ workspace, skillPath, refName });
      // Store in IndexedDB for next time
      await putFileTreeInCache(workspace, skillPath, refName, result);
      return result;
    },
    enabled: Boolean(workspace && skillPath),
    staleTime: 5 * 60 * 1000,
  });

  const tree = useMemo(() => buildFileTree(files ?? []), [files]);

  if (isLoading) {
    return (
      <div className="px-3 py-2 text-[11px] text-[var(--fg-muted)]">Loading files…</div>
    );
  }

  if (!files?.length) {
    return (
      <div className="px-3 py-2 text-[11px] text-[var(--fg-muted)]">No files found</div>
    );
  }

  return (
    <div className="skill-file-tree">
      {tree.map((node) => (
        <FileTreeNode
          key={node.relativePath}
          node={node}
          depth={0}
          selectedFile={selectedFile}
          onSelectFile={onSelectFile}
          expandedDirs={expandedDirs}
          onToggleDir={handleToggleDir}
        />
      ))}
    </div>
  );
}
