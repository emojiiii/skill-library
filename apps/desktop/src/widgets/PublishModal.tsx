import { Button, Modal } from "@heroui/react";
import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, FileText, GitPullRequestArrow, Image as ImageIcon, Package } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent, RefObject } from "react";
import { useLocale } from "../hooks/useLocale";
import {
  type ChangedFile,
  type SkillAsset,
  type SkillVersion,
  compareSkillVersions,
} from "../lib/skill-library";
import { Pill } from "./Pill";

type VersionBump = "patch" | "minor" | "major";

type PublishDraft = {
  filePath: string;
  fileName: string;
  before: string;
  after: string;
};

const EMPTY_FILES: ChangedFile[] = [];
const DIFF_LINE_HEIGHT = 22;
const VIRTUAL_DIFF_THRESHOLD = 320;
const IMAGE_EXTS = new Set(["png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "bmp"]);
const BINARY_EXTS = new Set(["woff", "woff2", "ttf", "otf", "eot", "zip", "tar", "gz", "pdf", "mp3", "mp4", "wav", "avi", "mov"]);

function isImageFile(filename: string) {
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTS.has(ext);
}

function isBinaryFile(filename: string) {
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  return BINARY_EXTS.has(ext);
}

function bumpVersion(current: string, bump: VersionBump): string {
  const parts = current.split(".").map(Number);
  const [major = 0, minor = 0, patch = 0] = parts;
  switch (bump) {
    case "major": return `${major + 1}.0.0`;
    case "minor": return `${major}.${minor + 1}.0`;
    case "patch": return `${major}.${minor}.${patch + 1}`;
  }
}

// --- Diff line rendering ---

type DiffLineKind = "add" | "remove" | "context" | "hunk" | "meta";

type ParsedDiffLine = {
  kind: DiffLineKind;
  oldLine: number | null;
  newLine: number | null;
  prefix: string;
  content: string;
};

type DiffOp = {
  type: "context" | "remove" | "add";
  text: string;
  oldLine: number | null;
  newLine: number | null;
  oldHint: number;
  newHint: number;
};

function splitDiffText(value: string): string[] {
  if (!value) return [];
  const lines = value.replace(/\r\n/g, "\n").split("\n");
  if (lines[lines.length - 1] === "") lines.pop();
  return lines;
}

function createUnifiedDiff(filename: string, before: string, after: string): string {
  const oldLines = splitDiffText(before);
  const newLines = splitDiffText(after);

  if (before === after) {
    return "";
  }

  // Skills are usually small. For unexpectedly large files, avoid an expensive
  // LCS table and fall back to a full remove/add patch.
  if (oldLines.length * newLines.length > 180_000) {
    return [
      `@@ -1,${Math.max(oldLines.length, 1)} +1,${Math.max(newLines.length, 1)} @@`,
      ...oldLines.map((line) => `-${line}`),
      ...newLines.map((line) => `+${line}`),
    ].join("\n");
  }

  const dp = Array.from({ length: oldLines.length + 1 }, () =>
    new Array<number>(newLines.length + 1).fill(0),
  );
  for (let i = oldLines.length - 1; i >= 0; i -= 1) {
    for (let j = newLines.length - 1; j >= 0; j -= 1) {
      dp[i][j] =
        oldLines[i] === newLines[j]
          ? dp[i + 1][j + 1] + 1
          : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const ops: DiffOp[] = [];
  let i = 0;
  let j = 0;
  let oldNo = 1;
  let newNo = 1;
  while (i < oldLines.length && j < newLines.length) {
    if (oldLines[i] === newLines[j]) {
      ops.push({
        type: "context",
        text: oldLines[i],
        oldLine: oldNo,
        newLine: newNo,
        oldHint: oldNo,
        newHint: newNo,
      });
      i += 1;
      j += 1;
      oldNo += 1;
      newNo += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({
        type: "remove",
        text: oldLines[i],
        oldLine: oldNo,
        newLine: null,
        oldHint: oldNo,
        newHint: newNo,
      });
      i += 1;
      oldNo += 1;
    } else {
      ops.push({
        type: "add",
        text: newLines[j],
        oldLine: null,
        newLine: newNo,
        oldHint: oldNo,
        newHint: newNo,
      });
      j += 1;
      newNo += 1;
    }
  }
  while (i < oldLines.length) {
    ops.push({
      type: "remove",
      text: oldLines[i],
      oldLine: oldNo,
      newLine: null,
      oldHint: oldNo,
      newHint: newNo,
    });
    i += 1;
    oldNo += 1;
  }
  while (j < newLines.length) {
    ops.push({
      type: "add",
      text: newLines[j],
      oldLine: null,
      newLine: newNo,
      oldHint: oldNo,
      newHint: newNo,
    });
    j += 1;
    newNo += 1;
  }

  const context = 3;
  const changedIndexes = ops
    .map((op, index) => (op.type === "context" ? -1 : index))
    .filter((index) => index >= 0);
  const ranges: Array<[number, number]> = [];
  for (const index of changedIndexes) {
    const start = Math.max(0, index - context);
    const end = Math.min(ops.length - 1, index + context);
    const last = ranges[ranges.length - 1];
    if (last && start <= last[1] + 1) last[1] = Math.max(last[1], end);
    else ranges.push([start, end]);
  }

  const patch: string[] = [];
  for (const [start, end] of ranges) {
    const hunk = ops.slice(start, end + 1);
    const oldStart = hunk.find((op) => op.oldLine != null)?.oldLine ?? hunk[0]?.oldHint ?? 1;
    const newStart = hunk.find((op) => op.newLine != null)?.newLine ?? hunk[0]?.newHint ?? 1;
    const oldCount = hunk.filter((op) => op.type !== "add").length;
    const newCount = hunk.filter((op) => op.type !== "remove").length;
    patch.push(`@@ -${oldStart},${oldCount} +${newStart},${newCount} @@`);
    for (const op of hunk) {
      const prefix = op.type === "add" ? "+" : op.type === "remove" ? "-" : " ";
      patch.push(`${prefix}${op.text}`);
    }
  }

  return patch.join("\n");
}

function parseUnifiedDiff(patch: string): ParsedDiffLine[] {
  let oldLine = 0;
  let newLine = 0;
  return splitDiffText(patch).map((line) => {
    const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
    if (hunk) {
      oldLine = Number(hunk[1]);
      newLine = Number(hunk[2]);
      return { kind: "hunk", oldLine: null, newLine: null, prefix: "", content: line };
    }
    if (line.startsWith("+") && !line.startsWith("+++")) {
      const parsed = { kind: "add" as const, oldLine: null, newLine, prefix: "+", content: line.slice(1) };
      newLine += 1;
      return parsed;
    }
    if (line.startsWith("-") && !line.startsWith("---")) {
      const parsed = { kind: "remove" as const, oldLine, newLine: null, prefix: "-", content: line.slice(1) };
      oldLine += 1;
      return parsed;
    }
    if (line.startsWith(" ")) {
      const parsed = { kind: "context" as const, oldLine, newLine, prefix: " ", content: line.slice(1) };
      oldLine += 1;
      newLine += 1;
      return parsed;
    }
    return { kind: "meta", oldLine: null, newLine: null, prefix: "", content: line };
  });
}

function DiffLine({ line }: { line: ParsedDiffLine }) {
  return (
    <div className={`publish-diff-line publish-diff-line--${line.kind}`}>
      <span className="publish-diff-line__no">{line.oldLine ?? ""}</span>
      <span className="publish-diff-line__no">{line.newLine ?? ""}</span>
      <span className="publish-diff-line__prefix">{line.prefix}</span>
      <span className="publish-diff-line__content">{line.content || " "}</span>
    </div>
  );
}

function fileStatusLabel(file: ChangedFile, t: (key: string) => string) {
  if (file.status === "added") return t("publish.added");
  if (file.status === "removed") return t("publish.removed");
  return t("publish.modified");
}

function FileIcon({ filename }: { filename: string }) {
  return isImageFile(filename) ? (
    <ImageIcon size={13} className="text-[var(--fg-muted)]" />
  ) : (
    <FileText size={13} className="text-[var(--fg-muted)]" />
  );
}

function DiffMinimap({
  lines,
  viewportRef,
}: {
  lines: ParsedDiffLine[];
  viewportRef: RefObject<HTMLDivElement | null>;
}) {
  const minimapRef = useRef<HTMLDivElement>(null);
  const markers = useMemo(() => {
    const changed = lines
      .map((line, index) => ({ line, index }))
      .filter(({ line }) => line.kind === "add" || line.kind === "remove" || line.kind === "hunk");
    const stride = Math.max(1, Math.ceil(changed.length / 500));
    return changed.filter((_, index) => index % stride === 0);
  }, [lines]);

  const scrollToClientY = (clientY: number) => {
    const minimap = minimapRef.current;
    const viewport = viewportRef.current;
    if (!minimap || !viewport) return;
    const rect = minimap.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (clientY - rect.top) / Math.max(rect.height, 1)));
    viewport.scrollTop = ratio * Math.max(0, viewport.scrollHeight - viewport.clientHeight);
  };

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    scrollToClientY(event.clientY);
    const handleMove = (moveEvent: PointerEvent) => scrollToClientY(moveEvent.clientY);
    const handleUp = () => {
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleUp);
    };
    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleUp);
  };

  if (lines.length < 20 || !markers.length) return null;

  return (
    <div
      ref={minimapRef}
      className="publish-diff-minimap"
      onPointerDown={handlePointerDown}
      aria-hidden="true"
    >
      {markers.map(({ line, index }) => (
        <span
          key={`${index}:${line.kind}`}
          className={`publish-diff-minimap__marker publish-diff-minimap__marker--${line.kind}`}
          style={{ top: `${(index / Math.max(lines.length - 1, 1)) * 100}%` }}
        />
      ))}
    </div>
  );
}

function DiffLines({ lines }: { lines: ParsedDiffLine[] }) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(520);
  const virtualized = lines.length > VIRTUAL_DIFF_THRESHOLD;

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const update = () => setViewportHeight(viewport.clientHeight || 520);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(viewport);
    return () => observer.disconnect();
  }, []);

  const start = virtualized
    ? Math.max(0, Math.floor(scrollTop / DIFF_LINE_HEIGHT) - 24)
    : 0;
  const count = virtualized
    ? Math.ceil(viewportHeight / DIFF_LINE_HEIGHT) + 48
    : lines.length;
  const visibleLines = virtualized ? lines.slice(start, start + count) : lines;

  return (
    <div
      ref={viewportRef}
      className="publish-diff-viewport"
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <DiffMinimap lines={lines} viewportRef={viewportRef} />
      <div
        className="publish-diff-lines"
        style={virtualized ? { height: lines.length * DIFF_LINE_HEIGHT } : undefined}
      >
        <div
          className={virtualized ? "absolute left-0 right-0 top-0" : undefined}
          style={virtualized ? { transform: `translateY(${start * DIFF_LINE_HEIGHT}px)` } : undefined}
        >
          {visibleLines.map((line, index) => (
            <DiffLine
              key={`${start + index}:${line.kind}:${line.content}`}
              line={line}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

export function InlineFileDiff({ file }: { file: ChangedFile }) {
  const { t } = useLocale();
  const lines = useMemo(() => (file.patch ? parseUnifiedDiff(file.patch) : []), [file.patch]);
  const [collapsed, setCollapsed] = useState(false);

  return (
    <section className="overflow-hidden rounded-lg border border-[var(--line)] bg-[var(--bg-elevated)]">
      <button
        type="button"
        className="flex w-full items-center justify-between gap-3 border-b border-[var(--line)] px-3 py-2 text-left hover:bg-[var(--bg-soft)]"
        onClick={() => setCollapsed((value) => !value)}
      >
        <div className="flex min-w-0 items-center gap-2">
          {collapsed ? <ChevronRight size={13} /> : <ChevronDown size={13} />}
          <FileIcon filename={file.filename} />
          <span className="truncate font-mono text-[12px] text-[var(--fg)]">{file.filename}</span>
        </div>
        <Pill tone={file.status === "removed" ? "danger" : file.status === "added" ? "success" : "warning"}>
          {fileStatusLabel(file, t)}
        </Pill>
      </button>
      {collapsed ? null : (
      <div className="bg-[var(--diff-bg)]">
        {isBinaryFile(file.filename) ? (
          <div className="px-6 py-10 text-center text-[12px] text-[var(--diff-muted)]">
            {t("publish.binaryNoPreview")}
          </div>
        ) : file.patch ? (
          <DiffLines lines={lines} />
        ) : (
          <div className="px-6 py-10 text-center text-[12px] text-[var(--diff-muted)]">{t("publish.noDiff")}</div>
        )}
      </div>
      )}
    </section>
  );
}

// --- Main modal ---

export function PublishModal({
  open,
  onOpenChange,
  asset,
  workspace,
  versions,
  selectedRef,
  localDraft,
  hasLocalChanges,
  onPublish,
  publishPending,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  asset: SkillAsset | null;
  workspace: string;
  versions: SkillVersion[];
  selectedRef?: string;
  localDraft?: PublishDraft | null;
  hasLocalChanges?: boolean;
  onPublish: (input: { bump: VersionBump; message: string }) => void;
  publishPending: boolean;
}) {
  const { t } = useLocale();
  const [bump, setBump] = useState<VersionBump>("patch");
  const [message, setMessage] = useState("");

  // Reset on open
  useEffect(() => {
    if (open) {
      setBump("patch");
      setMessage("");
    }
  }, [open]);

  const currentVersion = asset?.manifest.version ?? "0.0.0";
  const nextVersion = bumpVersion(currentVersion, bump);

  // Get diff between latest version and HEAD
  const latestVersion = versions.length > 0 ? versions[0] : null;
  const comparison = useQuery({
    queryKey: ["publish-comparison", workspace, asset?.path, latestVersion?.sha, selectedRef],
    queryFn: () =>
      compareSkillVersions({
        workspace,
        skillPath: asset?.path ?? "",
        from: latestVersion!.sha,
        to: selectedRef ?? "HEAD",
      }),
    // Only compare when we have a previous version sha to diff against
    enabled: open && Boolean(asset?.path && workspace && latestVersion?.sha),
    staleTime: 60 * 1000,
  });

  const remoteFiles = comparison.data?.files ?? EMPTY_FILES;
  const files = useMemo<ChangedFile[]>(() => {
    if (!localDraft) return remoteFiles;
    return [
      {
        filename: localDraft.filePath,
        status: "modified",
        patch: createUnifiedDiff(localDraft.filePath, localDraft.before, localDraft.after),
      },
    ];
  }, [localDraft, remoteFiles]);
  const stats = useMemo(() => {
    let added = 0, removed = 0, modified = 0;
    for (const f of files) {
      if (f.status === "added") added++;
      else if (f.status === "removed") removed++;
      else modified++;
    }
    return { added, removed, modified, total: files.length };
  }, [files]);

  if (!asset) return null;

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="lg">
          <Modal.Dialog className="flex h-[min(920px,calc(100vh-48px))] w-[min(1480px,calc(100vw-48px))] max-w-none flex-col rounded-[12px] bg-[var(--bg-elevated)] outline-none">
            <Modal.CloseTrigger />
            <Modal.Header className="shrink-0 border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold tracking-tight flex items-center gap-2">
                <Package size={16} />
                {t("publish.title")} "{asset.manifest.name}"
              </Modal.Heading>
              <div className="mt-1 text-[12px] text-[var(--fg-muted)]">
                <span className="font-mono">{workspace}</span>
                {" · "}
                <span className="font-mono">{asset.path}</span>
              </div>
            </Modal.Header>

            <Modal.Body className="flex min-h-0 flex-1 overflow-hidden p-0">
              {/* Left: version bump + message */}
              <div className="flex w-[360px] shrink-0 flex-col overflow-y-auto border-r border-[var(--line)]">
                <div className="space-y-5 px-5 py-4">
                  {/* Version bump */}
                  <section>
                    <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                      {t("publish.version")}
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-[12.5px] font-mono text-[var(--fg-muted)]">{currentVersion}</span>
                      <span className="text-[var(--fg-muted)]">→</span>
                      <span className="text-[13px] font-mono font-semibold text-[var(--brand)]">{nextVersion}</span>
                    </div>
                    <div className="mt-2.5 flex gap-2">
                      {(["patch", "minor", "major"] as const).map((type) => (
                        <button
                          key={type}
                          type="button"
                          onClick={() => setBump(type)}
                          className={`rounded-md border px-3 py-1.5 text-[12px] font-medium transition-colors ${
                            bump === type
                              ? "border-[var(--brand)] bg-[var(--brand-soft)] text-[var(--brand-fg)]"
                              : "border-[var(--line)] bg-[var(--bg-elevated)] text-[var(--fg-muted)] hover:bg-[var(--bg-soft)]"
                          }`}
                        >
                          {type}
                        </button>
                      ))}
                    </div>
                  </section>

                  {/* Commit message */}
                  <section>
                    <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                      {t("publish.releaseNotes")}
                    </div>
                    <textarea
                      value={message}
                      onChange={(e) => setMessage(e.target.value)}
                      placeholder={t("publish.releaseNotes.placeholder")}
                      rows={6}
                      className="w-full rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2 text-[13px] outline-none resize-y focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)] placeholder:text-[var(--fg-muted)]"
                    />
                  </section>
                </div>

                {/* Action buttons pinned to bottom */}
                <div className="mt-auto flex gap-2 border-t border-[var(--line)] px-5 py-3">
                  <Button
                    variant="outline"
                    onPress={() => onOpenChange(false)}
                    isDisabled={publishPending}
                    className="flex-1"
                  >
                    {t("publish.cancel")}
                  </Button>
                  <Button
                    onPress={() => onPublish({ bump, message })}
                    isPending={publishPending}
                    isDisabled={!message.trim()}
                    className="flex-1"
                  >
                    <GitPullRequestArrow size={14} />
                    {publishPending ? t("publish.submitting") : `${t("publish.submit")} ${nextVersion}`}
                  </Button>
                </div>
              </div>

              {/* Right: file changes */}
              <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
                <div className="flex items-center justify-between border-b border-[var(--line)] px-4 py-2.5">
                  <div className="text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                    {t("publish.fileChanges")}
                  </div>
                  {stats.total > 0 && (
                    <div className="flex items-center gap-2 text-[11px]">
                      {stats.added > 0 && <span className="text-[var(--success)]">+{stats.added}</span>}
                      {stats.modified > 0 && <span className="text-[var(--warning)]">~{stats.modified}</span>}
                      {stats.removed > 0 && <span className="text-[var(--danger)]">-{stats.removed}</span>}
                    </div>
                  )}
                </div>

                <div className="min-h-0 flex-1 overflow-y-auto bg-[var(--bg-soft)] px-4 py-3">
                  {files.length ? (
                    <div className="space-y-4">
                      {files.map((file) => (
                        <InlineFileDiff key={file.filename} file={file} />
                      ))}
                    </div>
                  ) : comparison.isFetching ? (
                    <div className="rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-4 py-6 text-center text-[12px] text-[var(--fg-muted)]">
                      {t("publish.loading")}
                    </div>
                  ) : comparison.error ? (
                    <div className="rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
                      {comparison.error instanceof Error ? comparison.error.message : t("publish.loadFailed")}
                    </div>
                  ) : (
                    <div className="rounded-md border border-dashed border-[var(--line)] bg-[var(--bg-elevated)] px-4 py-6 text-center text-[12px] text-[var(--fg-muted)]">
                      {hasLocalChanges
                        ? t("publish.localChangesDetected")
                        : latestVersion?.sha
                          ? t("publish.noChanges")
                          : t("publish.firstPublish")}
                    </div>
                  )}
                </div>
              </div>
            </Modal.Body>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
