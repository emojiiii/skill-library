import { Button, Modal } from "@heroui/react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, FileText, GitPullRequestArrow, Image as ImageIcon, Package } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import {
  type ChangedFile,
  type PublishPreview,
  type SkillAsset,
  type SkillComparison,
  type SkillVersion,
  compareSkillVersions,
  previewPublish,
  readSkillFile,
} from "../lib/teamai";
import { Pill } from "./Pill";

type VersionBump = "patch" | "minor" | "major";

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

function DiffLine({ line, index }: { line: string; index: number }) {
  let cls = "diff-line";
  if (line.startsWith("+") && !line.startsWith("+++")) cls += " diff-line--added";
  else if (line.startsWith("-") && !line.startsWith("---")) cls += " diff-line--removed";
  else if (line.startsWith("@@")) cls += " diff-line--hunk";

  return (
    <div key={index} className={cls}>
      <span className="diff-line__content">{line}</span>
    </div>
  );
}

function FilePatch({ file, workspace, skillPath, fromRef, toRef }: {
  file: ChangedFile;
  workspace: string;
  skillPath: string;
  fromRef?: string;
  toRef?: string;
}) {
  const { t } = useLocale();
  const [expanded, setExpanded] = useState(false);
  const isImage = isImageFile(file.filename);
  const isBin = isBinaryFile(file.filename);

  // For images, fetch before/after content
  const beforeImage = useQuery({
    queryKey: ["publish-img-before", workspace, skillPath, file.filename, fromRef],
    queryFn: () => readSkillFile({ workspace, filePath: `${skillPath}/${file.filename}`, refName: fromRef }),
    enabled: expanded && isImage && file.status !== "added",
    staleTime: 5 * 60 * 1000,
  });

  const afterImage = useQuery({
    queryKey: ["publish-img-after", workspace, skillPath, file.filename, toRef],
    queryFn: () => readSkillFile({ workspace, filePath: `${skillPath}/${file.filename}`, refName: toRef }),
    enabled: expanded && isImage && file.status !== "removed",
    staleTime: 5 * 60 * 1000,
  });

  const ext = file.filename.split(".").pop()?.toLowerCase() ?? "";
  const statusColor = file.status === "added" ? "text-[var(--success)]" : file.status === "removed" ? "text-[var(--danger)]" : "text-[var(--warning)]";
  const statusLabel = file.status === "added" ? t("publish.added") : file.status === "removed" ? t("publish.removed") : t("publish.modified");

  return (
    <div className="rounded-md border border-[var(--line)] overflow-hidden">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-[var(--bg-soft)] transition-colors"
      >
        {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        {isImage ? <ImageIcon size={13} className="text-[var(--fg-muted)]" /> : <FileText size={13} className="text-[var(--fg-muted)]" />}
        <span className="flex-1 truncate font-mono text-[12px]">{file.filename}</span>
        <span className={`text-[11px] font-medium ${statusColor}`}>{statusLabel}</span>
      </button>

      {expanded && (
        <div className="border-t border-[var(--line)]">
          {isBin ? (
            <div className="px-4 py-6 text-center text-[12px] text-[var(--fg-muted)]">
              {t("publish.binaryNoPreview")}
            </div>
          ) : isImage ? (
            <div className="px-4 py-3">
              <div className="flex items-start gap-4">
                {file.status !== "added" && (
                  <div className="flex-1 min-w-0">
                    <div className="mb-1.5 text-[11px] font-medium text-[var(--danger)]">Before</div>
                    {beforeImage.isPending ? (
                      <div className="h-24 rounded border border-dashed border-[var(--line)] grid place-items-center text-[11px] text-[var(--fg-muted)]">{t("publish.imageLoading")}</div>
                    ) : beforeImage.data ? (
                      <img
                        src={`data:image/${ext};base64,${beforeImage.data.content}`}
                        alt="before"
                        className="max-h-48 rounded border border-[var(--line)] object-contain"
                      />
                    ) : (
                      <div className="h-24 rounded border border-dashed border-[var(--line)] grid place-items-center text-[11px] text-[var(--fg-muted)]">{t("publish.imageLoadFailed")}</div>
                    )}
                  </div>
                )}
                {file.status !== "removed" && (
                  <div className="flex-1 min-w-0">
                    <div className="mb-1.5 text-[11px] font-medium text-[var(--success)]">After</div>
                    {afterImage.isPending ? (
                      <div className="h-24 rounded border border-dashed border-[var(--line)] grid place-items-center text-[11px] text-[var(--fg-muted)]">{t("publish.imageLoading")}</div>
                    ) : afterImage.data ? (
                      <img
                        src={`data:image/${ext};base64,${afterImage.data.content}`}
                        alt="after"
                        className="max-h-48 rounded border border-[var(--line)] object-contain"
                      />
                    ) : (
                      <div className="h-24 rounded border border-dashed border-[var(--line)] grid place-items-center text-[11px] text-[var(--fg-muted)]">{t("publish.imageLoadFailed")}</div>
                    )}
                  </div>
                )}
              </div>
            </div>
          ) : file.patch ? (
            <div className="overflow-x-auto bg-[var(--bg)] text-[11.5px] font-mono leading-[1.6]">
              {file.patch.split("\n").map((line, i) => (
                <DiffLine key={i} line={line} index={i} />
              ))}
            </div>
          ) : (
            <div className="px-4 py-4 text-center text-[12px] text-[var(--fg-muted)]">
              {t("publish.noDiff")}
            </div>
          )}
        </div>
      )}
    </div>
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
  onPublish,
  publishPending,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  asset: SkillAsset | null;
  workspace: string;
  versions: SkillVersion[];
  selectedRef?: string;
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
    queryKey: ["publish-comparison", workspace, asset?.path, latestVersion?.sha],
    queryFn: () =>
      compareSkillVersions({
        workspace,
        skillPath: asset?.path ?? "",
        from: latestVersion!.sha,
        to: "HEAD",
      }),
    // Only compare when we have a previous version sha to diff against
    enabled: open && Boolean(asset?.path && workspace && latestVersion?.sha),
    staleTime: 60 * 1000,
  });

  const files = comparison.data?.files ?? [];
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
          <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none max-h-[85vh] w-[min(900px,90vw)] flex flex-col">
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4 shrink-0">
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
              <div className="flex w-[320px] shrink-0 flex-col border-r border-[var(--line)] overflow-y-auto">
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
                  <Button variant="outline" onPress={() => onOpenChange(false)} className="flex-1">
                    {t("publish.cancel")}
                  </Button>
                  <Button
                    onPress={() => onPublish({ bump, message })}
                    isPending={publishPending}
                    isDisabled={!message.trim()}
                    className="flex-1"
                  >
                    <GitPullRequestArrow size={14} />
                    {t("publish.submit")} {nextVersion}
                  </Button>
                </div>
              </div>

              {/* Right: file changes */}
              <div className="flex min-w-0 flex-1 flex-col overflow-y-auto">
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

                <div className="flex-1 overflow-y-auto px-4 py-3">
                  {!latestVersion?.sha ? (
                    <div className="rounded-md border border-dashed border-[var(--line)] px-4 py-6 text-center text-[12px] text-[var(--fg-muted)]">
                      {t("publish.firstPublish")}
                    </div>
                  ) : comparison.isFetching ? (
                    <div className="rounded-md border border-[var(--line)] px-4 py-6 text-center text-[12.5px] text-[var(--fg-muted)]">
                      {t("publish.loading")}
                    </div>
                  ) : comparison.error ? (
                    <div className="rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
                      {comparison.error instanceof Error ? comparison.error.message : t("publish.loadFailed")}
                    </div>
                  ) : files.length === 0 ? (
                    <div className="rounded-md border border-dashed border-[var(--line)] px-4 py-6 text-center text-[12px] text-[var(--fg-muted)]">
                      {t("publish.noChanges")}
                    </div>
                  ) : (
                    <div className="space-y-2">
                      {files.map((file) => (
                        <FilePatch
                          key={file.filename}
                          file={file}
                          workspace={workspace}
                          skillPath={asset.path}
                          fromRef={latestVersion?.name}
                          toRef={selectedRef}
                        />
                      ))}
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
