import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { MessageSquare, Plus, SmilePlus } from "lucide-react";
import { useLocale } from "../hooks/useLocale";
import { useEffect, useRef, useState, useCallback } from "react";
import { createPortal } from "react-dom";
import ReactMarkdown from "react-markdown";
import {
  addDiscussionComment,
  createSkillDiscussion,
  getDiscussionByNumber,
  getDiscussionComments,
  listSkillDiscussions,
  toggleDiscussionReaction,
  removeDiscussionReaction,
  type DiscussionComment,
  type DiscussionInfo,
  type DiscussionsStatus,
  type ReactionGroup,
} from "../lib/skill-library";
import {
  getDiscussionsEnabledCache,
  setDiscussionsEnabledCache,
  getDiscussionMappingCache,
  setDiscussionMappingCache,
  clearDiscussionMappingCache,
} from "../lib/workspaceCache";
import { githubRepoPath, workspaceProviderId } from "../lib/providers";
import { formatError, formatRelativeTime } from "../utils/format";

// GitHub reaction content → emoji mapping
const REACTION_EMOJI: Record<string, string> = {
  THUMBS_UP: "👍",
  THUMBS_DOWN: "👎",
  LAUGH: "😄",
  HOORAY: "🎉",
  CONFUSED: "😕",
  HEART: "❤️",
  ROCKET: "🚀",
  EYES: "👀",
};

const ALL_REACTIONS = Object.keys(REACTION_EMOJI);

function defaultDiscussionStatus(
  workspace: string,
  enabled: boolean,
  discussions: DiscussionInfo[],
): DiscussionsStatus {
  const providerId = workspaceProviderId(workspace);
  return {
    enabled,
    supported: providerId === "github.com" || providerId === "github",
    providerId,
    providerName: providerId === "github.com" || providerId === "github" ? "GitHub" : providerId,
    providerKind: providerId === "github.com" || providerId === "github" ? "github" : providerId,
    discussions,
  };
}

/** Emoji picker rendered via portal to avoid overflow clipping */
function EmojiPickerPortal({
  anchorRef,
  isPending,
  reactionMap,
  onToggle,
  onClose,
}: {
  anchorRef: React.RefObject<HTMLButtonElement | null>;
  isPending: boolean;
  reactionMap: Map<string, ReactionGroup>;
  onToggle: (content: string, remove: boolean) => void;
  onClose: () => void;
}) {
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  useEffect(() => {
    if (anchorRef.current) {
      const rect = anchorRef.current.getBoundingClientRect();
      setPos({ top: rect.bottom + 6, left: rect.left });
    }
  }, [anchorRef]);

  // Close on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (anchorRef.current?.contains(e.target as Node)) return;
      onClose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [onClose, anchorRef]);

  if (!pos) return null;

  return createPortal(
    <div
      className="fixed z-[9999] rounded-lg border border-[var(--line)] bg-[var(--bg-elevated)] shadow-lg p-2"
      style={{ top: pos.top, left: pos.left }}
    >
      <div className="grid grid-cols-4 gap-1">
        {ALL_REACTIONS.map((content) => {
          const group = reactionMap.get(content);
          const viewerReacted = group?.viewerHasReacted ?? false;
          return (
            <button
              key={content}
              type="button"
              disabled={isPending}
              onClick={() => {
                onToggle(content, viewerReacted);
                onClose();
              }}
              className={`flex items-center justify-center w-8 h-8 rounded-md text-[18px] transition-colors ${
                viewerReacted
                  ? "bg-[var(--brand-soft)] ring-1 ring-[var(--brand)]"
                  : "hover:bg-[var(--bg-hover)]"
              }`}
              title={content.replace(/_/g, " ").toLowerCase()}
            >
              {REACTION_EMOJI[content]}
            </button>
          );
        })}
      </div>
    </div>,
    document.body,
  );
}

function ReactionBar({
  reactionMap,
  isPending,
  onToggle,
}: {
  reactionMap: Map<string, ReactionGroup>;
  isPending: boolean;
  onToggle: (content: string, remove: boolean) => void;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const { t } = useLocale();

  const handleClose = useCallback(() => setPickerOpen(false), []);

  // Reactions that have at least 1 count
  const activeReactions = ALL_REACTIONS.filter((c) => (reactionMap.get(c)?.count ?? 0) > 0);

  return (
    <div className="flex items-center gap-1.5 flex-wrap">
      {activeReactions.map((content) => {
        const group = reactionMap.get(content)!;
        const viewerReacted = group.viewerHasReacted;
        return (
          <button
            key={content}
            type="button"
            disabled={isPending}
            onClick={() => onToggle(content, viewerReacted)}
            className={`inline-flex items-center gap-1 rounded-md border px-2 py-1 text-[13px] transition-all duration-150 select-none ${
              viewerReacted
                ? "border-[var(--brand)] bg-[var(--brand-soft)] shadow-sm"
                : "border-[var(--line)] bg-[var(--bg-elevated)] hover:border-[var(--fg-muted)] hover:bg-[var(--bg-hover)]"
            }`}
            title={content.replace(/_/g, " ").toLowerCase()}
          >
            <span className="text-[14px] leading-none">{REACTION_EMOJI[content]}</span>
            <span className={`text-[11px] font-medium tabular-nums ${viewerReacted ? "text-[var(--brand)]" : "text-[var(--fg-muted)]"}`}>
              {group.count}
            </span>
          </button>
        );
      })}

      {/* Add reaction button */}
      <button
        ref={btnRef}
        type="button"
        disabled={isPending}
        onClick={() => setPickerOpen(!pickerOpen)}
        className={`inline-flex items-center justify-center w-7 h-7 rounded-md border transition-all duration-150 ${
          pickerOpen
            ? "border-[var(--brand)] bg-[var(--brand-soft)] text-[var(--brand)]"
            : "border-dashed border-[var(--line)] text-[var(--fg-muted)] hover:border-[var(--brand)] hover:text-[var(--brand)] hover:bg-[var(--bg-hover)]"
        }`}
        title={t("reaction.add")}
      >
        <SmilePlus size={14} />
      </button>

      {pickerOpen && (
        <EmojiPickerPortal
          anchorRef={btnRef}
          isPending={isPending}
          reactionMap={reactionMap}
          onToggle={onToggle}
          onClose={handleClose}
        />
      )}
    </div>
  );
}

/** Markdown body renderer */
function MarkdownBody({ content }: { content: string }) {
  return (
    <div className="markdown-comment-body">
      <ReactMarkdown
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

/** Discussion body (first post) — shown at the top like GitHub */
function DiscussionBody({
  discussion,
  isPending,
  onToggleReaction,
}: {
  discussion: DiscussionInfo;
  isPending: boolean;
  onToggleReaction: (content: string, remove: boolean) => void;
}) {
  const { locale } = useLocale();
  const reactionMap = new Map<string, ReactionGroup>(
    (discussion.reactions ?? []).map((r) => [r.content, r]),
  );

  return (
    <div className="discussion-body mb-4 pb-4 border-b border-[var(--line)]">
      {/* Author header */}
      <div className="flex items-center gap-2 mb-2">
        <div className="w-6 h-6 rounded-full overflow-hidden flex-shrink-0">
          {discussion.bodyAuthorAvatar ? (
            <img
              src={discussion.bodyAuthorAvatar}
              alt={discussion.bodyAuthor}
              className="w-full h-full object-cover"
            />
          ) : (
            <div className="w-full h-full bg-[var(--bg-hover)] flex items-center justify-center text-[10px] font-medium text-[var(--fg-muted)]">
              {discussion.bodyAuthor.slice(0, 2).toUpperCase()}
            </div>
          )}
        </div>
        <span className="text-[12px] font-medium text-[var(--fg)]">
          @{discussion.bodyAuthor}
        </span>
        <span className="text-[11px] text-[var(--fg-muted)]">
          {formatRelativeTime(discussion.createdAt, locale)}
        </span>
      </div>

      {/* Body content — Markdown */}
      {discussion.body ? (
        <div className="mb-3 pl-8">
          <MarkdownBody content={discussion.body} />
        </div>
      ) : null}

      {/* Reactions */}
      <div className="pl-8">
        <ReactionBar
          reactionMap={reactionMap}
          isPending={isPending}
          onToggle={onToggleReaction}
        />
      </div>
    </div>
  );
}

function CommentItem({
  comment,
  workspace,
  isPending,
  onToggleReaction,
}: {
  comment: DiscussionComment;
  workspace: string;
  isPending: boolean;
  onToggleReaction: (commentId: string, content: string, remove: boolean) => void;
}) {
  const { locale } = useLocale();
  const reactionMap = new Map<string, ReactionGroup>(
    (comment.reactions ?? []).map((r) => [r.content, r]),
  );

  return (
    <div className="comment-item">
      <div className="comment-avatar">
        {comment.authorAvatar ? (
          <img src={comment.authorAvatar} alt={comment.author} />
        ) : (
          <div className="comment-avatar__fallback">
            {comment.author.slice(0, 2).toUpperCase()}
          </div>
        )}
      </div>
      <div className="comment-body">
        <div className="comment-header">
          <span className="comment-author">@{comment.author}</span>
          <span className="comment-date">{formatRelativeTime(comment.createdAt, locale)}</span>
        </div>
        <div className="comment-text">
          <MarkdownBody content={comment.body} />
        </div>
        <div className="mt-2">
          <ReactionBar
            reactionMap={reactionMap}
            isPending={isPending}
            onToggle={(content, remove) => onToggleReaction(comment.id, content, remove)}
          />
        </div>
      </div>
    </div>
  );
}

function CommentInput({
  workspace,
  discussionId,
  onSuccess,
}: {
  workspace: string;
  discussionId: string;
  onSuccess: () => void;
}) {
  const [body, setBody] = useState("");
  const [tab, setTab] = useState<"write" | "preview">("write");
  const { t } = useLocale();

  const submit = useMutation({
    mutationFn: () => addDiscussionComment({ workspace, discussionId, body }),
    onSuccess: () => {
      setBody("");
      setTab("write");
      onSuccess();
    },
  });

  return (
    <div className="mt-3">
      {/* Bordered input container */}
      <div className="rounded-md border border-[var(--line)] overflow-hidden">
        {/* Tabs */}
        <div className="flex items-center gap-0 border-b border-[var(--line)] bg-[var(--bg-base)]">
          <button
            type="button"
            onClick={() => setTab("write")}
            className={`px-3 py-1.5 text-[12px] font-medium border-b-2 transition-colors ${
              tab === "write"
                ? "border-[var(--brand)] text-[var(--fg)]"
                : "border-transparent text-[var(--fg-muted)] hover:text-[var(--fg)]"
            }`}
          >
            {t("comment.tab.write")}
          </button>
          <button
            type="button"
            onClick={() => setTab("preview")}
            className={`px-3 py-1.5 text-[12px] font-medium border-b-2 transition-colors ${
              tab === "preview"
                ? "border-[var(--brand)] text-[var(--fg)]"
                : "border-transparent text-[var(--fg-muted)] hover:text-[var(--fg)]"
            }`}
          >
            {t("comment.tab.preview")}
          </button>
        </div>

        {/* Content area */}
        {tab === "write" ? (
          <div>
            <textarea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder={t("comment.placeholder")}
              rows={4}
              className="block w-full resize-none border-none bg-[var(--bg-elevated)] px-3 py-2.5 text-[13px] outline-none placeholder:text-[var(--fg-muted)]"
            />
            <div className="px-3 py-1.5 text-[11px] text-[var(--fg-muted)] bg-[var(--bg-elevated)] border-t border-dashed border-[var(--line)]">
              {t("comment.markdownHint")}
            </div>
          </div>
        ) : (
          <div className="min-h-[100px] px-3 py-2.5 bg-[var(--bg-elevated)]">
            {body.trim() ? (
              <MarkdownBody content={body} />
            ) : (
              <p className="text-[12px] text-[var(--fg-muted)] italic">{t("comment.preview.empty")}</p>
            )}
          </div>
        )}
      </div>

      {/* Submit button — outside the box */}
      <div className="flex items-center justify-end mt-2">
        <button
          type="button"
          disabled={!body.trim() || submit.isPending}
          onClick={() => submit.mutate()}
          className="rounded-md bg-[var(--brand)] px-3.5 py-1.5 text-[12px] font-medium text-white transition-opacity disabled:opacity-40 hover:opacity-90"
        >
          {submit.isPending ? t("comment.submitting") : t("comment.submit")}
        </button>
      </div>
    </div>
  );
}

function CreateWithCommentInput({
  isPending,
  onSubmit,
}: {
  isPending: boolean;
  onSubmit: (body: string) => void;
}) {
  const [body, setBody] = useState("");
  const [tab, setTab] = useState<"write" | "preview">("write");
  const { t } = useLocale();

  return (
    <div className="mt-4 w-full">
      <div className="text-[11px] text-[var(--fg-muted)] mb-1.5">{t("comment.firstHint")}</div>
      <div className="rounded-md border border-[var(--line)] overflow-hidden">
        {/* Tabs */}
        <div className="flex items-center gap-0 border-b border-[var(--line)] bg-[var(--bg-base)]">
          <button
            type="button"
            onClick={() => setTab("write")}
            className={`px-3 py-1.5 text-[12px] font-medium border-b-2 transition-colors ${
              tab === "write"
                ? "border-[var(--brand)] text-[var(--fg)]"
                : "border-transparent text-[var(--fg-muted)] hover:text-[var(--fg)]"
            }`}
          >
            {t("comment.tab.write")}
          </button>
          <button
            type="button"
            onClick={() => setTab("preview")}
            className={`px-3 py-1.5 text-[12px] font-medium border-b-2 transition-colors ${
              tab === "preview"
                ? "border-[var(--brand)] text-[var(--fg)]"
                : "border-transparent text-[var(--fg-muted)] hover:text-[var(--fg)]"
            }`}
          >
            {t("comment.tab.preview")}
          </button>
        </div>

        {/* Content area */}
        {tab === "write" ? (
          <div>
            <textarea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder={t("comment.placeholder")}
              rows={3}
              disabled={isPending}
              className="block w-full resize-none border-none bg-[var(--bg-elevated)] px-3 py-2.5 text-[13px] outline-none placeholder:text-[var(--fg-muted)] disabled:opacity-50"
            />
            <div className="px-3 py-1.5 text-[11px] text-[var(--fg-muted)] bg-[var(--bg-elevated)] border-t border-dashed border-[var(--line)]">
              {t("comment.markdownHint")}
            </div>
          </div>
        ) : (
          <div className="min-h-[76px] px-3 py-2.5 bg-[var(--bg-elevated)]">
            {body.trim() ? (
              <MarkdownBody content={body} />
            ) : (
              <p className="text-[12px] text-[var(--fg-muted)] italic">{t("comment.preview.empty")}</p>
            )}
          </div>
        )}
      </div>

      {/* Submit button — outside the box */}
      <div className="flex items-center justify-end mt-2">
        <button
          type="button"
          disabled={!body.trim() || isPending}
          onClick={() => onSubmit(body.trim())}
          className="rounded-md bg-[var(--brand)] px-3.5 py-1.5 text-[12px] font-medium text-white transition-opacity disabled:opacity-40 hover:opacity-90"
        >
          {isPending ? t("comment.submitting") : t("comment.submit")}
        </button>
      </div>
    </div>
  );
}

export function SkillComments({
  workspace,
  skillId,
  skillPath,
}: {
  workspace: string;
  skillId: string;
  skillPath?: string;
}) {
  const queryClient = useQueryClient();
  const { t } = useLocale();

  // Cached discussions enabled status avoids a first-load disabled-state flash.
  const [cachedEnabled, setCachedEnabled] = useState<boolean | null>(null);
  const [cachedForKey, setCachedForKey] = useState(`${workspace}:${skillId}`);

  // Synchronously reset when workspace/skillId changes (prevents stale flash)
  const currentKey = `${workspace}:${skillId}`;
  if (cachedForKey !== currentKey) {
    setCachedForKey(currentKey);
    setCachedEnabled(null);
  }

  useEffect(() => {
    let cancelled = false;
    if (workspace) {
      getDiscussionsEnabledCache(workspace).then((v) => {
        if (!cancelled && v !== null) setCachedEnabled(v);
      });
    }
    return () => { cancelled = true; };
  }, [workspace]);

  // Smart discussion fetching:
  // 1. Check SQLite cache for skillId → discussion number mapping
  // 2. If positive cache hit → fetch that single discussion directly (1 API call)
  // 3. If negative cache hit → return "no discussion" without any API call
  // 4. If no cache / expired → full scan, then cache the result (positive or negative)
  const discussions = useQuery({
    queryKey: ["skill-discussions", workspace, skillId],
    queryFn: async () => {
      // Try cached mapping first
      const cached = await getDiscussionMappingCache(workspace, skillId);
      if (cached) {
        // Negative cache: we previously confirmed no discussion exists
        if (cached.discussionId === null) {
          return defaultDiscussionStatus(workspace, true, []);
        }

        // Positive cache: fetch single discussion by number
        const info = await getDiscussionByNumber({
          workspace,
          discussionNumber: cached.discussionNumber!,
        });
        if (info) {
          return defaultDiscussionStatus(workspace, true, [info]);
        }
        // Cached mapping is stale (discussion deleted?) — clear and fall through
        void clearDiscussionMappingCache(workspace, skillId);
      }

      // Slow path: full scan
      const result = await listSkillDiscussions({ workspace, skillIds: [skillId] });

      // Cache the result (positive or negative)
      if (result.enabled) {
        if (result.discussions.length > 0) {
          const d = result.discussions[0];
          void setDiscussionMappingCache(workspace, skillId, {
            discussionId: d.id,
            discussionNumber: d.number,
            cachedAt: Date.now(),
          });
        } else {
          // Negative cache: no discussion found for this skill
          void setDiscussionMappingCache(workspace, skillId, {
            discussionId: null,
            discussionNumber: null,
            cachedAt: Date.now(),
          });
        }
      }

      return result;
    },
    enabled: Boolean(workspace && skillId),
    staleTime: 5 * 60 * 1000,
  });

  // Sync cache when API responds — only when data is fresh for the current workspace
  const discussionsDataRef = useRef(discussions.data);
  discussionsDataRef.current = discussions.data;
  useEffect(() => {
    // Only sync when discussions.data changes (not when workspace changes alone)
    // This prevents writing stale data from old workspace to new workspace's cache
    if (discussions.data && !discussions.isStale && !discussions.isFetching) {
      const enabled = discussions.data.enabled;
      setCachedEnabled(enabled);
      setDiscussionsEnabledCache(workspace, enabled);
    }
  }, [discussions.data, discussions.isStale, discussions.isFetching, workspace]);

  const discussion: DiscussionInfo | undefined = discussions.data?.discussions[0];

  const comments = useQuery({
    queryKey: ["discussion-comments", workspace, discussion?.number],
    queryFn: () =>
      getDiscussionComments({ workspace, discussionNumber: discussion!.number }),
    enabled: Boolean(discussion?.number),
    staleTime: 2 * 60 * 1000,
  });

  // Discussion-level reaction toggle
  const reactMutation = useMutation({
    mutationFn: ({ content, remove }: { content: string; remove: boolean }) =>
      remove
        ? removeDiscussionReaction({ workspace, discussionId: discussion!.id, content })
        : toggleDiscussionReaction({ workspace, discussionId: discussion!.id, content }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["skill-discussions", workspace, skillId] });
    },
  });

  // Comment-level reaction toggle (uses same API — subjectId is the comment node ID)
  const commentReactMutation = useMutation({
    mutationFn: ({ commentId, content, remove }: { commentId: string; content: string; remove: boolean }) =>
      remove
        ? removeDiscussionReaction({ workspace, discussionId: commentId, content })
        : toggleDiscussionReaction({ workspace, discussionId: commentId, content }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["discussion-comments", workspace, discussion?.number] });
    },
  });

  const createDiscussion = useMutation({
    mutationFn: (firstComment?: string) =>
      createSkillDiscussion({ workspace, skillId, skillPath, body: firstComment || undefined }),
    onSuccess: (data) => {
      // Cache the new mapping immediately
      if (data && data.id && data.number) {
        void setDiscussionMappingCache(workspace, skillId, {
          discussionId: data.id,
          discussionNumber: data.number,
          cachedAt: Date.now(),
        });
      }
      queryClient.invalidateQueries({ queryKey: ["skill-discussions", workspace, skillId] });
    },
    onError: (err: unknown) => {
      const code = (err as { code?: string })?.code;
      if (code === "discussions_disabled") {
        setCachedEnabled(false);
        setDiscussionsEnabledCache(workspace, false);
        queryClient.invalidateQueries({ queryKey: ["skill-discussions", workspace, skillId] });
      }
    },
  });

  // Determine effective enabled state
  const effectiveEnabled = discussions.data ? discussions.data.enabled : cachedEnabled;
  const effectiveSupported = discussions.data?.supported ?? true;
  const providerName = discussions.data?.providerName ?? defaultDiscussionStatus(workspace, true, []).providerName;

  // Still loading and no cache
  if (effectiveEnabled === null && discussions.isLoading) {
    return (
      <div className="text-[12px] text-[var(--fg-muted)] py-4 text-center">
        {t("discussion.loading")}
      </div>
    );
  }

  if (!effectiveSupported) {
    return (
      <div className="empty-state">
        <div className="empty-state__title">{t("discussion.unsupported.title")}</div>
        <div className="text-center">
          <p>{t("discussion.unsupported.desc").replace("{provider}", providerName)}</p>
        </div>
      </div>
    );
  }

  if (effectiveEnabled === false) {
    const settingsUrl = `https://github.com/${githubRepoPath(workspace)}/settings`;
    return (
      <div className="empty-state">
        <div className="empty-state__title">{t("discussion.notEnabled.title")}</div>
        <div className="text-center">
          <p>{t("discussion.notEnabled.desc")}</p>
          <p className="mt-2 text-[11.5px]">{t("discussion.notEnabled.hint")}</p>
          <a
            href={settingsUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="mt-3 inline-block rounded-md bg-[var(--brand)] px-4 py-2 text-[12px] font-medium hover:bg-[var(--brand-hover)]"
            style={{ color: "white" }}
          >
            {t("discussion.notEnabled.openSettings")}
          </a>
          <button
            type="button"
            className="mt-2 block mx-auto text-[11px] text-[var(--brand)] hover:underline"
            onClick={() => discussions.refetch()}
          >
            {t("discussion.notEnabled.refresh")}
          </button>
        </div>
      </div>
    );
  }

  if (!discussion) {
    return (
      <div className="empty-state">
        <MessageSquare size={20} className="text-[var(--fg-muted)]" />
        <div className="empty-state__title">{t("discussion.noDiscussion.title")}</div>
        <div className="text-[12px] text-[var(--fg-muted)] mb-3">
          {t("discussion.noDiscussion.desc")}
        </div>
        {createDiscussion.error ? (
          <div className="text-[12px] text-[var(--danger)] mb-2">
            {formatError(createDiscussion.error)}
          </div>
        ) : null}
        <button
          type="button"
          disabled={createDiscussion.isPending}
          onClick={() => createDiscussion.mutate(undefined)}
          className="inline-flex items-center gap-1.5 rounded-md bg-[var(--brand)] px-4 py-2 text-[12px] font-medium text-white hover:bg-[var(--brand-hover)] disabled:opacity-50"
        >
          <Plus size={13} />
          {createDiscussion.isPending ? t("discussion.creating") : t("discussion.create")}
        </button>
        <CreateWithCommentInput
          isPending={createDiscussion.isPending}
          onSubmit={(body) => createDiscussion.mutate(body)}
        />
      </div>
    );
  }

  return (
    <div>
      {/* Discussion body (first post) */}
      <DiscussionBody
        discussion={discussion}
        isPending={reactMutation.isPending}
        onToggleReaction={(content, remove) => reactMutation.mutate({ content, remove })}
      />

      {/* Comment count + GitHub link */}
      <div className="flex items-center gap-3 mb-3">
        <span className="text-[12px] text-[var(--fg-muted)]">
          {discussion.commentCount} {t("comment.count")}
        </span>
        <a
          href={discussion.url}
          target="_blank"
          rel="noopener noreferrer"
          className="ml-auto text-[11px] text-[var(--brand)] hover:underline"
        >
          {t("discussion.viewOnGithub")}
        </a>
      </div>

      {/* Comments list */}
      {comments.isLoading ? (
        <div className="text-[12px] text-[var(--fg-muted)]">{t("discussion.loadingComments")}</div>
      ) : comments.error ? (
        <div className="text-[12px] text-[var(--danger)]">{formatError(comments.error)}</div>
      ) : comments.data?.length ? (
        <div>
          {comments.data.map((comment) => (
            <CommentItem
              key={comment.id}
              comment={comment}
              workspace={workspace}
              isPending={commentReactMutation.isPending}
              onToggleReaction={(commentId, content, remove) =>
                commentReactMutation.mutate({ commentId, content, remove })
              }
            />
          ))}
        </div>
      ) : (
        <div className="text-[12px] text-[var(--fg-muted)]">{t("comment.empty")}</div>
      )}

      {/* Comment input */}
      <CommentInput
        workspace={workspace}
        discussionId={discussion.id}
        onSuccess={() => {
          queryClient.invalidateQueries({
            queryKey: ["discussion-comments", workspace, discussion.number],
          });
        }}
      />
    </div>
  );
}
