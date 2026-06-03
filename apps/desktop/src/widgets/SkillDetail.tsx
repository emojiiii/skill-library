import { AlertDialog, Button, Tooltip } from "@heroui/react";
import { useQuery } from "@tanstack/react-query";
import {
  BellPlus,
  ChevronDown,
  ChevronRight,
  Files,
  GitPullRequestArrow,
  Maximize2,
  Minimize2,
  RefreshCw,
  RotateCcw,
  Share2,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import {
  listProviderInstances,
  previewPublish,
  readSkillFile,
  type FileContent,
  type SkillAsset,
  type SkillDetail as SkillDetailData,
} from "../lib/skill-library";
import { useLocale } from "../hooks/useLocale";
import { useLocalStorage } from "../hooks/useLocalStorage";
import { providerSupportsComments, workspaceProviderId } from "../lib/providers";
import { getFileContentFromCache, putFileContentInCache } from "../lib/workspaceCache";
import { effectiveRisk, riskTone } from "../utils/risk";
import { CodeEditor } from "./CodeEditor";
import { MarkdownEditor } from "./MarkdownEditor";
import { Pill } from "./Pill";
import { ResultBlock } from "./ResultBlock";
import { SegmentedTabs } from "./SegmentedTabs";
import { SkillComments } from "./SkillComments";
import { SkillCommitsTimeline } from "./SkillCommitsTimeline";
import { SkillFileTree } from "./SkillFileTree";
import { SkillRiskPanel } from "./SkillRiskPanel";

type SkillTab = "source" | "metadata" | "history" | "comments" | "risk";

type FileViewMode = "editor" | "markdown-preview" | "image" | "pdf" | "binary";

export type SkillPublishDraft = {
  filePath: string;
  fileName: string;
  before: string;
  after: string;
};

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
  onSelectFile,
  targets,
  setTargets,
  workspaceRef,
  onSubscribeClick,
  onPublishClick,
  onPublishDraftChange,
  canEditSource = true,
  onSyncClick,
  onRefresh,
  hasLocalChanges = false,
  publishResetKey = 0,
  publishResetValue,
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
  onSelectFile: (file: string | null) => void;
  targets: string[];
  setTargets: (targets: string[]) => void;
  workspaceRef: string;
  onSubscribeClick: () => void;
  onPublishClick?: () => void;
  onPublishDraftChange?: (draft: SkillPublishDraft | null) => void;
  canEditSource?: boolean;
  onSyncClick?: () => void;
  onRefresh?: () => void;
  hasLocalChanges?: boolean;
  publishResetKey?: number;
  publishResetValue?: string | null;
  publishResult: Awaited<ReturnType<typeof previewPublish>> | undefined;
  subscriptions: number;
}) {
  const { t } = useLocale();
  const activeAsset = detail?.asset ?? asset;
  const skillMarkdown = detail?.skill_markdown?.content;
  const [tab, setTab] = useLocalStorage<SkillTab>(`ws-ui:${workspaceRef}:tab`, "source");
  const [fullscreen, setFullscreen] = useState(false);
  const [fileTreeOpen, setFileTreeOpen] = useState(false);
  const [editorResetVersion, setEditorResetVersion] = useState(0);
  const [editorBaseline, setEditorBaseline] = useState<string | null>(null);
  const providerId = detail?.workspace.provider ?? workspaceProviderId(workspaceRef);
  const providerInstances = useQuery({
    queryKey: ["provider-instances"],
    queryFn: listProviderInstances,
    staleTime: 10 * 60 * 1000,
  });
  const providerInstance = providerInstances.data?.find(
    (instance) => instance.id.toLowerCase() === providerId.toLowerCase(),
  );
  const commentsSupported = providerSupportsComments(providerInstance, providerId);

  // Reset internal state when skill changes
  useEffect(() => {
    setFullscreen(false);
    setFileTreeOpen(false);
  }, [asset.manifest.id]);

  useEffect(() => {
    if (!commentsSupported && tab === "comments") {
      setTab("source");
    }
  }, [commentsSupported, setTab, tab]);

  const riskLevel = effectiveRisk(activeAsset.manifest);
  const readOnlySource = !canEditSource;

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
  const currentFilePath = selectedFile ?? `${activeAsset.path}/SKILL.md`;
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
    setEditorBaseline(null);
    setFileTreeOpen(false);
  }, [asset.manifest.id, selectedFile]);

  useEffect(() => {
    if (!publishResetKey) return;
    setEditedContent(publishResetValue ?? null);
    setMarkdownDirty(false);
    setEditorBaseline(publishResetValue ?? null);
  }, [publishResetKey, publishResetValue]);

  // Dirty depends on the editor: markdown uses the editor's own baseline signal
  // (MDXEditor normalizes content, so a text compare gives false positives);
  // code/plain files are WYSIWYG so an exact compare against the original works.
  const hasUnpublishedChanges =
    viewMode === "markdown-preview"
      ? markdownDirty
      : editedContent !== null && editedContent !== sourceContent;
  const hasPublishUpdate = hasUnpublishedChanges || hasLocalChanges;
  const publishDraftBaseline =
    viewMode === "markdown-preview" ? editorBaseline ?? sourceContent : sourceContent;

  useEffect(() => {
    if (!onPublishDraftChange) return;
    if (readOnlySource || !hasUnpublishedChanges) {
      onPublishDraftChange(null);
      return;
    }
    onPublishDraftChange({
      filePath: currentFilePath,
      fileName: currentFileName,
      before: publishDraftBaseline,
      after: editedContent ?? sourceContent,
    });
  }, [
    currentFileName,
    currentFilePath,
    editedContent,
    hasUnpublishedChanges,
    onPublishDraftChange,
    publishDraftBaseline,
    readOnlySource,
    sourceContent,
  ]);

  useEffect(() => {
    if (!readOnlySource) return;
    setEditedContent(null);
    setMarkdownDirty(false);
  }, [readOnlySource]);

  // Revert handler: reset editor content to original
  const handleRevert = useCallback(() => {
    setEditedContent(null);
    setMarkdownDirty(false);
    setShowRevertConfirm(false);
    setEditorResetVersion((value) => value + 1);
  }, []);

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
            content={viewMode === "markdown-preview" ? sourceContent : editedContent ?? sourceContent}
            fileName={currentFileName}
            viewMode={viewMode}
            loading={sourceLoading}
            onChange={setEditedContent}
            onBaselineChange={setEditorBaseline}
            onDirtyChange={setMarkdownDirty}
            readOnly={readOnlySource}
            resetKey={editorResetVersion}
            baselineResetKey={publishResetKey}
            baselineResetValue={publishResetValue}
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
                {t(`risk.level.${riskLevel}`)}
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

      </div>

      {/* Tabs + content */}
      <div className="min-h-0 flex-1 overflow-hidden">
        <div className="border-b border-[var(--line)] bg-[var(--bg-elevated)] px-5">
          <SegmentedTabs<SkillTab>
            tabs={[
              { id: "source", label: t("skill.tab.source") },
              { id: "metadata", label: t("skill.tab.metadata") },
              { id: "history", label: t("skill.tab.history") },
              ...(commentsSupported ? [{ id: "comments" as const, label: t("skill.tab.comments") }] : []),
              { id: "risk", label: t("skill.tab.risk") },
            ]}
            active={tab}
            onChange={setTab}
          />
        </div>

        <div className="scroll-area max-h-full">
          {tab === "source" ? (
            <div className="relative flex h-full min-h-[400px] flex-col">
              {/* File path bar + actions */}
              <div className="flex items-center justify-between border-b border-[var(--line)] bg-[var(--bg-soft)] px-5 py-1.5">
                <button
                  type="button"
                  className="flex min-w-0 items-center gap-1.5 rounded-md px-1.5 py-1 text-left text-[11px] font-mono text-[var(--fg-muted)] hover:bg-[var(--bg-active)] hover:text-[var(--fg)]"
                  onClick={() => setFileTreeOpen((value) => !value)}
                >
                  {fileTreeOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                  <Files size={13} className="shrink-0" />
                  <span className="truncate">{selectedFile ?? `${activeAsset.path}/SKILL.md`}</span>
                </button>
                <div className="flex items-center gap-1.5">
                  {/* Revert button — left of publish */}
                  {!readOnlySource && hasUnpublishedChanges && (
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
                        {hasPublishUpdate && (
                          <span className="absolute -right-0.5 -top-0.5 size-2.5 rounded-full border border-[var(--bg-elevated)] bg-[var(--warning)]" />
                        )}
                      </Button>
                      <Tooltip.Content>
                        {hasPublishUpdate ? t("skill.publishHasUpdates") : t("skill.publish")}
                      </Tooltip.Content>
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
                  content={viewMode === "markdown-preview" ? sourceContent : editedContent ?? sourceContent}
                  fileName={currentFileName}
                  viewMode={viewMode}
                  loading={sourceLoading}
                  onChange={setEditedContent}
                  onBaselineChange={setEditorBaseline}
                  onDirtyChange={setMarkdownDirty}
                  resetKey={editorResetVersion}
                  baselineResetKey={publishResetKey}
                  baselineResetValue={publishResetValue}
                  readOnly={readOnlySource}
                />
              </div>
              {fileTreeOpen ? (
                <div className="absolute right-4 top-12 z-20 max-h-[min(520px,calc(100%-64px))] w-[340px] overflow-hidden rounded-lg border border-[var(--line)] bg-[var(--bg-elevated)] shadow-2xl">
                  <div className="border-b border-[var(--line)] px-3 py-2 text-[11px] font-semibold uppercase tracking-wide text-[var(--fg-muted)]">
                    {t("skill.files")}
                  </div>
                  <div className="max-h-[min(470px,calc(100vh-260px))] overflow-y-auto py-1">
                    <SkillFileTree
                      workspace={workspaceRef}
                      skillPath={activeAsset.path}
                      refName={selectedRef}
                      selectedFile={selectedFile}
                      onSelectFile={(file) => {
                        onSelectFile(file);
                        setFileTreeOpen(false);
                      }}
                    />
                  </div>
                </div>
              ) : null}
            </div>
          ) : null}

          {/* Revert confirmation */}
          <AlertDialog.Backdrop isOpen={showRevertConfirm} onOpenChange={setShowRevertConfirm}>
            <AlertDialog.Container size="sm">
              <AlertDialog.Dialog className="sm:max-w-[420px]">
                <AlertDialog.CloseTrigger />
                <AlertDialog.Header>
                  <AlertDialog.Icon status="danger">
                    <RotateCcw className="size-5" />
                  </AlertDialog.Icon>
                  <AlertDialog.Heading>{t("skill.revert.confirm.title")}</AlertDialog.Heading>
                </AlertDialog.Header>
                <AlertDialog.Body>
                  <p>{t("skill.revert.confirm.desc")}</p>
                </AlertDialog.Body>
                <AlertDialog.Footer>
                  <Button slot="close" variant="tertiary">
                    {t("common.cancel")}
                  </Button>
                  <Button slot="close" variant="danger-soft" onPress={handleRevert}>
                    <RotateCcw size={14} />
                    {t("skill.revert.confirm.btn")}
                  </Button>
                </AlertDialog.Footer>
              </AlertDialog.Dialog>
            </AlertDialog.Container>
          </AlertDialog.Backdrop>

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
  onBaselineChange,
  onDirtyChange,
  readOnly = false,
  resetKey = 0,
  baselineResetKey = 0,
  baselineResetValue,
}: {
  content: string;
  fileName: string;
  viewMode: FileViewMode;
  loading: boolean;
  onChange?: (value: string) => void;
  onBaselineChange?: (value: string) => void;
  onDirtyChange?: (dirty: boolean) => void;
  readOnly?: boolean;
  resetKey?: number;
  baselineResetKey?: number;
  baselineResetValue?: string | null;
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
      if (readOnly) {
        return (
          <pre className="markdown-preview h-full max-h-none whitespace-pre-wrap font-sans text-[13px] leading-6">
            {content}
          </pre>
        );
      }
      return (
        <div className="h-full">
          <MarkdownEditor
            key={resetKey}
            initialValue={content}
            onChange={onChange}
            onBaselineChange={onBaselineChange}
            baselineResetKey={baselineResetKey}
            baselineResetValue={baselineResetValue}
            onDirtyChange={onDirtyChange}
          />
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
          <CodeEditor value={content} fileName={fileName} readOnly={readOnly} onChange={onChange} />
        </div>
      );
  }
}
