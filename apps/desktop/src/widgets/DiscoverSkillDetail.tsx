import { useQuery } from "@tanstack/react-query";
import { Download, FileText, ShieldAlert } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Spinner } from "@heroui/react";
import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import type { RegistrySkill } from "../lib/registry";
import {
  cacheSkillPackage,
  readSkillFile,
  type FileContent,
  type SkillDetail as SkillDetailData,
} from "../lib/teamai";
import { getFileContentFromCache, putFileContentInCache } from "../lib/workspaceCache";
import { useLocale } from "../hooks/useLocale";
import { formatError } from "../utils/format";
import { effectiveRisk, riskLabel, riskTone } from "../utils/risk";
import { Pill } from "./Pill";
import { SkillFileTree } from "./SkillFileTree";
import { SkillSafetyCard } from "./SkillSafetyCard";

type FileViewMode = "markdown" | "image" | "pdf" | "binary" | "text";

const IMAGE_EXTS = new Set(["png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "svg"]);
const UNSUPPORTED_BINARY_EXTS = new Set([
  "ttf", "otf", "woff", "woff2", "eot",
  "pdf",
  "zip", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar",
  "wasm",
  "doc", "docx", "xls", "xlsx", "ppt", "pptx",
  "mp3", "mp4", "mov", "avi", "webm",
]);

function fileExt(fileName: string) {
  return fileName.split(".").pop()?.toLowerCase() ?? "";
}

function detectFileViewMode(fileName: string, isBinary: boolean): FileViewMode {
  const ext = fileExt(fileName);
  if (IMAGE_EXTS.has(ext)) return "image";
  if (ext === "pdf") return "pdf";
  if (UNSUPPORTED_BINARY_EXTS.has(ext)) return "binary";
  if (isBinary) {
    return "binary";
  }

  if (ext === "md" || ext === "mdx") return "markdown";
  return "text";
}

function shouldFetchFileForPreview(fileName: string) {
  const ext = fileExt(fileName);
  return IMAGE_EXTS.has(ext) || !UNSUPPORTED_BINARY_EXTS.has(ext);
}

function imageMime(fileName: string) {
  const ext = fileExt(fileName) || "png";
  if (ext === "jpg") return "image/jpeg";
  if (ext === "svg") return "image/svg+xml";
  return `image/${ext}`;
}

export function DiscoverSkillDetail({
  selected,
  detail,
  loading,
  error,
  installPending,
  onInstallClick,
}: {
  selected: RegistrySkill;
  detail: SkillDetailData | undefined;
  loading: boolean;
  error: unknown;
  installPending: boolean;
  onInstallClick: () => void;
}) {
  const { t } = useLocale();
  const manifest = detail?.asset.manifest ?? null;
  const skillPath = detail?.asset.path ?? null;
  const skillMarkdown = detail?.skill_markdown?.content ?? "";
  const defaultFile = skillPath ? `${skillPath}/SKILL.md` : null;
  const [selectedFile, setSelectedFile] = useState<string | null>(null);

  useEffect(() => {
    setSelectedFile(defaultFile);
  }, [selected.id, defaultFile]);

  useEffect(() => {
    if (!skillPath) return;
    void cacheSkillPackage({
      workspace: selected.source,
      skillPath,
    }).catch(() => undefined);
  }, [selected.source, skillPath]);

  const currentFileName = selectedFile?.split("/").pop() ?? "SKILL.md";

  const fileContent = useQuery({
    queryKey: ["discover-skill-file-content", selected.source, selectedFile],
    queryFn: async () => {
      const cached = await getFileContentFromCache(selected.source, selectedFile!);
      if (cached) {
        return {
          path: cached.filePath,
          content: cached.content,
          sha: "",
          encoding: cached.isBinary ? "base64" : "utf-8",
          isBinary: cached.isBinary,
        } as FileContent;
      }
      const result = await readSkillFile({
        workspace: selected.source,
        filePath: selectedFile!,
      });
      await putFileContentInCache(selected.source, selectedFile!, undefined, result);
      return result;
    },
    enabled: Boolean(
      selectedFile &&
      selectedFile !== defaultFile &&
      shouldFetchFileForPreview(currentFileName),
    ),
    staleTime: 10 * 60 * 1000,
  });

  const currentContent = selectedFile === defaultFile ? skillMarkdown : fileContent.data?.content ?? "";
  const currentIsBinary = selectedFile === defaultFile ? false : fileContent.data?.isBinary ?? false;
  const viewMode = useMemo(
    () => detectFileViewMode(currentFileName, currentIsBinary),
    [currentFileName, currentIsBinary],
  );
  const riskLevel = manifest ? effectiveRisk(manifest) : null;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-b border-[var(--line)] bg-[var(--bg-elevated)] px-6 py-4">
        <div className="flex items-start gap-4">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="truncate text-[15px] font-semibold text-[var(--fg)]">
                {manifest?.name ?? selected.name}
              </h2>
              {manifest?.version ? <Pill mono>v{manifest.version}</Pill> : null}
              {riskLevel ? (
                <Pill tone={riskTone[riskLevel] === "default" ? "default" : (riskTone[riskLevel] as never)}>
                  {riskLabel[riskLevel]}
                </Pill>
              ) : null}
            </div>
            <div className="mt-1 line-clamp-2 text-[12.5px] leading-[1.45] text-[var(--fg-muted)]">
              {manifest?.description || selected.source}
            </div>
            <div className="mt-2 truncate font-mono text-[11px] text-[var(--fg-muted)]">
              {selected.source}
            </div>
          </div>

          <Button
            size="sm"
            variant="secondary"
            className="h-8 shrink-0 rounded-full px-3.5"
            isDisabled={!manifest}
            isPending={installPending}
            onPress={onInstallClick}
          >
            <Download size={14} />
            {t("discover.installShort")}
          </Button>
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-1 overflow-hidden md:grid-cols-[248px_minmax(0,1fr)]">
        <aside className="flex min-h-0 max-h-[220px] flex-col border-b border-[var(--line)] bg-[var(--bg-elevated)] md:max-h-none md:border-b-0 md:border-r">
          <div className="flex h-10 shrink-0 items-center gap-2 border-b border-[var(--line)] bg-[var(--bg-soft)] px-4">
            <FileText size={13} className="text-[var(--fg-muted)]" />
            <span className="text-[11px] font-semibold uppercase tracking-wide text-[var(--fg-muted)]">
              {t("discover.files")}
            </span>
          </div>
          <div className="discover-file-tree scroll-area min-h-0 flex-1 px-2.5 py-3">
            {loading && !skillPath ? (
              <div className="flex items-center gap-2 px-3 py-2 text-[11px] text-[var(--fg-muted)]">
                <Spinner size="sm" />
                {t("common.loading")}
              </div>
            ) : skillPath ? (
              <SkillFileTree
                workspace={selected.source}
                skillPath={skillPath}
                selectedFile={selectedFile}
                onSelectFile={setSelectedFile}
              />
            ) : (
              <div className="px-3 py-2 text-[11px] text-[var(--fg-muted)]">
                {error ? formatError(error) : t("discover.detailUnavailable")}
              </div>
            )}
          </div>
          {manifest ? (
            <details className="shrink-0 border-t border-[var(--line)] bg-[var(--bg-soft)] px-4 py-3">
              <summary className="flex cursor-pointer items-center gap-2 text-[11.5px] font-medium text-[var(--fg-muted)] hover:text-[var(--fg)]">
                <ShieldAlert size={13} />
                {t("discover.safetyPermissions")}
              </summary>
              <div className="mt-2">
                <SkillSafetyCard manifest={manifest} />
              </div>
            </details>
          ) : null}
        </aside>

        <section className="flex min-h-0 flex-col bg-[var(--bg-elevated)]">
          <div className="flex h-10 shrink-0 items-center justify-between gap-3 border-b border-[var(--line)] bg-[var(--bg-soft)] px-5">
            <span className="truncate font-mono text-[11px] text-[var(--fg-muted)]">
              {selectedFile ?? (skillPath ? `${skillPath}/SKILL.md` : selected.skillId)}
            </span>
            <span className="shrink-0 text-[11px] font-medium uppercase tracking-wide text-[var(--fg-muted)]">
              {t("discover.preview")}
            </span>
          </div>
          <div className="scroll-area min-h-0 flex-1 bg-[var(--bg)]">
            <FileContentView
              content={currentContent}
              fileName={currentFileName}
              viewMode={viewMode}
              isBinary={currentIsBinary}
              loading={loading || fileContent.isLoading}
              error={fileContent.error}
            />
          </div>
        </section>
      </div>
    </div>
  );
}

function FileContentView({
  content,
  fileName,
  viewMode,
  isBinary,
  loading,
  error,
}: {
  content: string;
  fileName: string;
  viewMode: FileViewMode;
  isBinary: boolean;
  loading: boolean;
  error: unknown;
}) {
  const { t } = useLocale();

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center gap-2 p-8 text-[12px] text-[var(--fg-muted)]">
        <Spinner size="sm" />
        {t("skill.loadingFile")}
      </div>
    );
  }

  if (error) {
    return (
      <div className="m-4 rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-[12px] text-[var(--warning)]">
        {formatError(error)}
      </div>
    );
  }

  if (viewMode === "pdf") {
    return (
      <div className="flex h-full items-center justify-center p-8 text-[12px] text-[var(--fg-muted)]">
        {t("skill.pdfNotAvailable")}
      </div>
    );
  }

  if (viewMode === "binary") {
    return (
      <div className="flex h-full items-center justify-center p-8 text-[12px] text-[var(--fg-muted)]">
        {t("skill.binaryFile")}
      </div>
    );
  }

  if (!content) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-[12px] text-[var(--fg-muted)]">
        {t("discover.detailUnavailable")}
      </div>
    );
  }

  if (viewMode === "markdown") {
    return (
      <div className="mx-auto max-w-[760px] px-8 py-8">
        <DiscoverMarkdownPreview content={content} />
      </div>
    );
  }

  if (viewMode === "image") {
    const src = isBinary
      ? `data:${imageMime(fileName)};base64,${content}`
      : `data:${imageMime(fileName)};charset=utf-8,${encodeURIComponent(content)}`;
    return (
      <div className="flex h-full items-center justify-center p-6">
        <img
          src={src}
          alt={fileName}
          className="max-h-full max-w-full rounded-md border border-[var(--line)] bg-[var(--bg)]"
        />
      </div>
    );
  }

  return (
    <pre className="min-h-full overflow-auto bg-[var(--bg)] px-6 py-5 font-mono text-[12px] leading-[1.65] text-[var(--fg)]">
      <code>{content}</code>
    </pre>
  );
}

function DiscoverMarkdownPreview({ content }: { content: string }) {
  return (
    <div className="discover-markdown">
      <ReactMarkdown
        rehypePlugins={[rehypeSanitize]}
        components={{
          a: ({ href, children }) => (
            <a href={href} target="_blank" rel="noopener noreferrer">
              {children}
            </a>
          ),
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
