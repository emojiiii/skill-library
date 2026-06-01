import { Button, Modal, Tooltip } from "@heroui/react";
import { useQuery } from "@tanstack/react-query";
import {
  BellPlus,
  Download,
  GitPullRequestArrow,
  Maximize2,
  Minimize2,
  RefreshCw,
  RotateCcw,
  Share2,
  ShieldAlert,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import {
  installSkill,
  previewPublish,
  readSkillFile,
  type FileContent,
  type SkillAsset,
  type SkillDetail as SkillDetailData,
} from "../lib/teamai";
import { useLocale } from "../hooks/useLocale";
import { useLocalStorage } from "../hooks/useLocalStorage";
import { getFileContentFromCache, putFileContentInCache } from "../lib/workspaceCache";
import { effectiveRisk, permissionSummary, riskLabel, riskRequiresConfirmation, riskTone } from "../utils/risk";
import { CodeEditor } from "./CodeEditor";
import { MarkdownEditor } from "./MarkdownEditor";
import { Pill } from "./Pill";
import { ResultBlock } from "./ResultBlock";
import { SegmentedTabs } from "./SegmentedTabs";
import { SkillComments } from "./SkillComments";
import { SkillCommitsTimeline } from "./SkillCommitsTimeline";
import { SkillRiskPanel } from "./SkillRiskPanel";

type SkillTab = "source" | "metadata" | "history" | "comments" | "risk";

type FileViewMode = "editor" | "markdown-preview" | "image" | "pdf" | "binary";

function detectFileViewMode(fileName: string): FileViewMode {
  const ext = fileName.split(".").pop()?.toLowerCase() ?? "";
  // Markdown — rich preview + edit
  if (ext === "md") return "markdown-preview";
  // Images — view only
  if (["png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "bmp"].includes(ext)) return "image";
  // PDF — view only
  if (ext === "pdf") return "pdf";
  // Editable code/text files
  if ([
    "yaml", "yml", "json", "toml", "ts", "tsx", "js", "jsx",
    "py", "rs", "lua", "sh", "bash", "zsh", "fish",
    "css", "scss", "html", "xml", "sql", "graphql",
    "txt", "env", "gitignore", "dockerfile",
  ].includes(ext)) return "editor";
  // Binary-ish
  if (["doc", "docx", "xls", "xlsx", "ppt", "pptx", "zip", "tar", "gz", "wasm"].includes(ext)) return "binary";
  // Default: treat as text editor
  return "editor";
}

function getLanguageForExt(fileName: string): string {
  const ext = fileName.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    yaml: "yaml", yml: "yaml", json: "json", toml: "toml",
    ts: "typescript", tsx: "typescript", js: "javascript", jsx: "javascript",
    py: "python", rs: "rust", lua: "lua", sh: "bash", bash: "bash",
    css: "css", scss: "scss", html: "html", xml: "xml", sql: "sql",
  };
  return map[ext] ?? "plaintext";
}

export function SkillDetail({
  asset,
  detail,
  detailPending,
  detailError,
  selectedRef,
  setSelectedRef,
  selectedFile,
  targets,
  setTargets,
  workspaceRef,
  onSubscribeClick,
  onInstall,
  onPublish,
  onPublishClick,
  onSyncClick,
  onRefresh,
  installPending,
  publishPending,
  installResult,
  publishResult,
  subscriptions,
}: {
  asset: SkillAsset;
  detail: SkillDetailData | undefined;
  detailPending: boolean;
  detailError: Error | null;
  selectedRef: string | undefined;
  setSelectedRef: (ref: string | undefined) => void;
  selectedFile: string | null;
  targets: string[];
  setTargets: (targets: string[]) => void;
  workspaceRef: string;
  onSubscribeClick: () => void;
  onInstall: (confirmed?: boolean) => void;
  onPublish: () => void;
  onPublishClick?: () => void;
  onSyncClick?: () => void;
  onRefresh?: () => void;
  installPending: boolean;
  publishPending: boolean;
  installResult: Awaited<ReturnType<typeof installSkill>> | undefined;
  publishResult: Awaited<ReturnType<typeof previewPublish>> | undefined;
  subscriptions: number;
}) {
  const { t } = useLocale();
  const activeAsset = detail?.asset ?? asset;
  const skillMarkdown = detail?.skill_markdown?.content;
  const [pendingRiskAction, setPendingRiskAction] = useState<"install" | "publish" | null>(null);
  const [tab, setTab] = useLocalStorage<SkillTab>(`ws-ui:${workspaceRef}:tab`, "source");
  const [fullscreen, setFullscreen] = useState(false);

  // Reset internal state when skill changes
  useEffect(() => {
    setPendingRiskAction(null);
    setFullscreen(false);
  }, [asset.manifest.id]);

  const riskLevel = effectiveRisk(activeAsset.manifest);
  const requiresConfirmation = riskRequiresConfirmation(riskLevel);

  // Fetch selected file content (with filesystem persistent cache)
  const fileContent = useQuery({
    queryKey: ["skill-file-content", workspaceRef, selectedFile, selectedRef],
    queryFn: async () => {
      // Check filesystem cache first
      const cached = await getFileContentFromCache(workspaceRef, selectedFile!, selectedRef);
      if (cached) {
        return {
          path: cached.filePath,
          content: cached.content,
          sha: "",
          encoding: cached.isBinary ? "base64" : "utf-8",
          isBinary: cached.isBinary,
        } as FileContent;
      }
      // Cache miss — fetch from API
      const result = await readSkillFile({
        workspace: workspaceRef,
        filePath: selectedFile!,
        refName: selectedRef,
      });
      // Store in filesystem cache for next time
      await putFileContentInCache(workspaceRef, selectedFile!, selectedRef, result);
      return result;
    },
    enabled: Boolean(workspaceRef && selectedFile),
    staleTime: 10 * 60 * 1000,
  });

  // Determine source content and file type
  const currentFileName = selectedFile
    ? selectedFile.split("/").pop() ?? "file"
    : "SKILL.md";
  const sourceContent = selectedFile
    ? fileContent.data?.content ?? ""
    : skillMarkdown ?? "";
  const sourceLoading = selectedFile ? fileContent.isLoading : false;
  const isBinary = selectedFile ? fileContent.data?.isBinary ?? false : false;
  const viewMode = isBinary ? "binary" : detectFileViewMode(currentFileName);

  // Track local dirty state (editor content vs original cached content)
  const [editedContent, setEditedContent] = useState<string | null>(null);
  // For markdown files, MDXEditor re-serializes on mount so we can't compare its
  // output against the original text — the editor reports dirty against its own
  // normalized baseline instead.
  const [markdownDirty, setMarkdownDirty] = useState(false);
  const [showRevertConfirm, setShowRevertConfirm] = useState(false);

  // Reset edited content when skill or file changes
  useEffect(() => {
    setEditedContent(null);
    setMarkdownDirty(false);
  }, [asset.manifest.id, selectedFile]);

  // Dirty depends on the editor: markdown uses the editor's own baseline signal
  // (MDXEditor normalizes content, so a text compare gives false positives);
  // code/plain files are WYSIWYG so an exact compare against the original works.
  const hasUnpublishedChanges =
    viewMode === "markdown-preview"
      ? markdownDirty
      : editedContent !== null && editedContent !== sourceContent;

  // Revert handler: reset editor content to original
  const handleRevert = useCallback(() => {
    setEditedContent(null);
    setMarkdownDirty(false);
    setShowRevertConfirm(false);
  }, []);

  const startRiskAction = (action: "install" | "publish") => {
    if (requiresConfirmation) {
      setPendingRiskAction(action);
      return;
    }
    if (action === "install") {
      onInstall(false);
    } else {
      onPublish();
    }
  };

  const confirmRiskAction = () => {
    if (pendingRiskAction === "install") {
      onInstall(true);
    } else if (pendingRiskAction === "publish") {
      onPublish();
    }
    setPendingRiskAction(null);
  };

  void targets;
  void setTargets;
  void subscriptions;

  // Fullscreen overlay for source
  if (fullscreen) {
    return (
      <div className="fixed inset-0 z-[100] flex flex-col bg-[var(--bg)]">
        <div className="flex items-center justify-between border-b border-[var(--line)] bg-[var(--bg-elevated)] px-5 py-2">
          <span className="text-[12px] font-mono text-[var(--fg-muted)]">{selectedFile ?? `${activeAsset.path}/SKILL.md`}</span>
          <button
            type="button"
            className="rounded-md p-1.5 text-[var(--fg-muted)] hover:bg-[var(--bg-active)] hover:text-[var(--fg)]"
            onClick={() => setFullscreen(false)}
          >
            <Minimize2 size={15} />
          </button>
        </div>
        <div className="min-h-0 flex-1">
          <SourceViewer
            content={editedContent ?? sourceContent}
            fileName={currentFileName}
            viewMode={viewMode}
            loading={sourceLoading}
            onChange={setEditedContent}
            onDirtyChange={setMarkdownDirty}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Compact header — no bottom border; flows into the tab bar below */}
      <div className="bg-[var(--bg-elevated)] px-5 py-3.5">
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <h2 className="truncate text-[15px] font-semibold text-[var(--fg)]">
                {activeAsset.manifest.name}
              </h2>
              <Pill mono>v{activeAsset.manifest.version}</Pill>
              <Pill tone={riskTone[riskLevel] === "default" ? "default" : (riskTone[riskLevel] as never)}>
                {riskLabel[riskLevel]}
              </Pill>
              {onRefresh && (
                <Tooltip delay={0}>
                  <Button
                    isIconOnly
                    size="sm"
                    variant="ghost"
                    className="min-w-7 h-7 text-[var(--fg-muted)] hover:text-[var(--brand)]"
                    onPress={onRefresh}
                    isPending={detailPending}
                  >
                    <RefreshCw size={13} />
                  </Button>
                  <Tooltip.Content>{t("common.refresh")}</Tooltip.Content>
                </Tooltip>
              )}
            </div>
            <div className="mt-0.5 truncate text-[11.5px] text-[var(--fg-muted)]">
              {activeAsset.manifest.description || activeAsset.path}
            </div>
          </div>

          {/* Primary actions */}
          <div className="flex shrink-0 items-center gap-1.5">
            <Tooltip delay={0}>
              <Button isIconOnly size="sm" variant="secondary" onPress={onSubscribeClick}>
                <BellPlus size={14} />
              </Button>
              <Tooltip.Content>{t("skill.subscribe")}</Tooltip.Content>
            </Tooltip>
            <Tooltip delay={0}>
              <Button isIconOnly size="sm" variant="secondary" onPress={() => startRiskAction("install")} isPending={installPending}>
                <Download size={14} />
              </Button>
              <Tooltip.Content>{t("skill.install")}</Tooltip.Content>
            </Tooltip>
            {onSyncClick ? (
              <Tooltip delay={0}>
                <Button isIconOnly size="sm" variant="secondary" onPress={onSyncClick}>
                  <Share2 size={14} />
                </Button>
                <Tooltip.Content>{t("skill.syncTo")}</Tooltip.Content>
              </Tooltip>
            ) : null}
          </div>
        </div>


        {/* Risk confirmation banner */}
        {pendingRiskAction ? (
          <div className="mt-2.5 rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2">
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2 text-[12px]">
                <ShieldAlert className="shrink-0 text-[var(--warning)]" size={13} />
                <span className="text-[var(--warning)]">
                  {t("common.confirm")} {pendingRiskAction === "install" ? t("skill.riskConfirm.install") : t("skill.riskConfirm.publish")} · {riskLabel[riskLevel].toLowerCase()} risk · {permissionSummary(activeAsset.manifest)}
                </span>
              </div>
              <div className="flex shrink-0 gap-1.5">
                <Button size="sm" variant="outline" onPress={() => setPendingRiskAction(null)}>
                  {t("skill.cancel")}
                </Button>
                <Button size="sm" variant="secondary" onPress={confirmRiskAction} isPending={installPending || publishPending}>
                  {t("skill.confirm")}
                </Button>
              </div>
            </div>
          </div>
        ) : null}
      </div>

      {/* Tabs + content */}
      <div className="min-h-0 flex-1 overflow-hidden">
        <div className="border-b border-[var(--line)] bg-[var(--bg-elevated)] px-5">
          <SegmentedTabs<SkillTab>
            tabs={[
              { id: "source", label: t("skill.tab.source") },
              { id: "metadata", label: t("skill.tab.metadata") },
              { id: "history", label: t("skill.tab.history") },
              { id: "comments", label: t("skill.tab.comments") },
              { id: "risk", label: t("skill.tab.risk") },
            ]}
            active={tab}
            onChange={setTab}
          />
        </div>

        <div className="scroll-area max-h-full">
          {tab === "source" ? (
            <div className="flex h-full min-h-[400px] flex-col">
              {/* File path bar + actions */}
              <div className="flex items-center justify-between border-b border-[var(--line)] bg-[var(--bg-soft)] px-5 py-1.5">
                <span className="truncate text-[11px] font-mono text-[var(--fg-muted)]">
                  {selectedFile ?? `${activeAsset.path}/SKILL.md`}
                </span>
                <div className="flex items-center gap-1.5">
                  {/* Revert button — left of publish */}
                  {hasUnpublishedChanges && (
                    <Tooltip delay={0}>
                      <Button
                        isIconOnly
                        size="sm"
                        variant="ghost"
                        className="min-w-9 h-9 text-[var(--fg-muted)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
                        onPress={() => setShowRevertConfirm(true)}
                      >
                        <RotateCcw size={15} />
                      </Button>
                      <Tooltip.Content>{t("skill.revert")}</Tooltip.Content>
                    </Tooltip>
                  )}
                  {onPublishClick && (
                    <Tooltip delay={0}>
                      <Button
                        isIconOnly
                        size="sm"
                        variant="ghost"
                        className="relative overflow-visible min-w-9 h-9 text-[var(--fg-muted)] hover:bg-[var(--brand-soft)] hover:text-[var(--brand)]"
                        onPress={onPublishClick}
                      >
                        <GitPullRequestArrow size={15} />
                        {hasUnpublishedChanges && (
                          <span className="absolute -right-0.5 -top-0.5 size-2.5 rounded-full bg-[var(--danger)]" />
                        )}
                      </Button>
                      <Tooltip.Content>{t("skill.publish")}</Tooltip.Content>
                    </Tooltip>
                  )}
                  <Tooltip delay={0}>
                    <Button
                      isIconOnly
                      size="sm"
                      variant="ghost"
                      className="min-w-9 h-9 text-[var(--fg-muted)] hover:bg-[var(--brand-soft)] hover:text-[var(--brand)]"
                      onPress={() => setFullscreen(true)}
                    >
                      <Maximize2 size={15} />
                    </Button>
                    <Tooltip.Content>{t("skill.fullscreen")}</Tooltip.Content>
                  </Tooltip>
                </div>
              </div>

              {/* Content area */}
              <div className="min-h-0 flex-1">
                <SourceViewer
                  content={editedContent ?? sourceContent}
                  fileName={currentFileName}
                  viewMode={viewMode}
                  loading={sourceLoading}
                  onChange={setEditedContent}
                  onDirtyChange={setMarkdownDirty}
                />
              </div>
            </div>
          ) : null}

          {/* Revert confirmation modal */}
          <Modal isOpen={showRevertConfirm} onOpenChange={setShowRevertConfirm}>
            <Modal.Backdrop>
              <Modal.Container size="sm">
                <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none w-[min(420px,90vw)]">
                  <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
                    <Modal.Heading className="text-[15px] font-semibold tracking-tight flex items-center gap-2">
                      <RotateCcw size={16} className="text-[var(--danger)]" />
                      {t("skill.revert.confirm.title")}
                    </Modal.Heading>
                  </Modal.Header>
                  <Modal.Body className="px-5 py-4">
                    <p className="text-[13px] text-[var(--fg-muted)]">
                      {t("skill.revert.confirm.desc")}
                    </p>
                  </Modal.Body>
                  <div className="flex justify-end gap-2 border-t border-[var(--line)] px-5 py-3">
                    <Button variant="outline" onPress={() => setShowRevertConfirm(false)}>
                      {t("common.cancel")}
                    </Button>
                    <Button variant="danger-soft" onPress={handleRevert}>
                      <RotateCcw size={14} />
                      {t("skill.revert.confirm.btn")}
                    </Button>
                  </div>
                </Modal.Dialog>
              </Modal.Container>
            </Modal.Backdrop>
          </Modal>

          {tab === "metadata" ? (
            <div className="px-5 py-4">
              <div className="space-y-4">
                <div>
                  <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wide text-[var(--fg-muted)]">{t("skill.name")}</label>
                  <input
                    defaultValue={activeAsset.manifest.name}
                    className="w-full rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2 text-[13px] text-[var(--fg)] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                  />
                </div>
                <div>
                  <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wide text-[var(--fg-muted)]">{t("skill.description")}</label>
                  <textarea
                    defaultValue={activeAsset.manifest.description}
                    rows={4}
                    className="w-full resize-none rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2 text-[13px] text-[var(--fg)] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                  />
                </div>
              </div>

              {installResult ? <ResultBlock title={t("skill.installReport")} value={installResult} /> : null}
              {publishResult ? <ResultBlock title={t("skill.publishPreview")} value={publishResult} /> : null}
            </div>
          ) : null}

          {tab === "history" ? (
            <div className="px-5 py-4">
              <SkillCommitsTimeline
                workspace={workspaceRef}
                skillPath={activeAsset.path}
                refName={selectedRef}
              />
            </div>
          ) : null}

          {tab === "comments" ? (
            <div className="px-5 py-4">
              <SkillComments
                key={`${workspaceRef}:${activeAsset.manifest.id}`}
                workspace={workspaceRef}
                skillId={activeAsset.manifest.id}
                skillPath={activeAsset.path}
              />
            </div>
          ) : null}

          {tab === "risk" ? (
            <div className="px-5 py-4">
              <SkillRiskPanel
                manifest={activeAsset.manifest}
                skillPath={activeAsset.path}
                workspace={workspaceRef}
                refName={selectedRef}
                workspacePermission={detail?.workspace.permission}
              />
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

/** Renders file content based on detected file type */
function SourceViewer({
  content,
  fileName,
  viewMode,
  loading,
  onChange,
  onDirtyChange,
}: {
  content: string;
  fileName: string;
  viewMode: FileViewMode;
  loading: boolean;
  onChange?: (value: string) => void;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const { t } = useLocale();
  if (loading) {
    return (
      <div className="flex items-center justify-center p-8 text-[12px] text-[var(--fg-muted)]">
        {t("skill.loadingFile")}
      </div>
    );
  }

  switch (viewMode) {
    case "markdown-preview":
      return (
        <div className="h-full">
          <MarkdownEditor initialValue={content} onChange={onChange} onDirtyChange={onDirtyChange} />
        </div>
      );

    case "image":
      return (
        <div className="flex items-center justify-center p-8">
          <img
            src={`data:image/${fileName.split(".").pop()};base64,${content}`}
            alt={fileName}
            className="max-h-[500px] max-w-full rounded-md border border-[var(--line)]"
          />
        </div>
      );

    case "pdf":
      return (
        <div className="flex items-center justify-center p-8 text-[12px] text-[var(--fg-muted)]">
          {t("skill.pdfNotAvailable")}
        </div>
      );

    case "binary":
      return (
        <div className="flex items-center justify-center p-8 text-[12px] text-[var(--fg-muted)]">
          {t("skill.binaryFile")}
        </div>
      );

    case "editor":
    default:
      return (
        <div className="h-full">
          <CodeEditor value={content} fileName={fileName} onChange={onChange} />
        </div>
      );
  }
}
