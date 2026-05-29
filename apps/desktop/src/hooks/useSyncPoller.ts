import { useCallback, useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  checkWorkspaceHead,
  diffWorkspaceSince,
  isTauri,
  type SkillAsset,
} from "../lib/teamai";
import {
  loadWorkspaceCache,
  updateCacheHead,
  invalidateSkillsInCache,
} from "../lib/workspaceCache";

export interface SyncPollerOptions {
  /** Current workspace full_name (e.g. "owner/repo") */
  workspace: string | null;
  /** Currently known skill paths in this workspace */
  skillPaths: string[];
  /** Whether the app window is focused */
  focused?: boolean;
  /** Whether polling is enabled */
  enabled?: boolean;
}

interface BackoffState {
  consecutiveNoChange: number;
  lastCheckAt: number;
}

function getInterval(backoff: BackoffState, focused: boolean): number {
  if (!focused) return 30 * 60 * 1000; // 30 min when backgrounded

  const n = backoff.consecutiveNoChange;
  if (n < 3) return 2 * 60 * 1000;   // 2 min — active, recent changes
  if (n < 6) return 5 * 60 * 1000;   // 5 min — stable
  return 10 * 60 * 1000;              // 10 min — very stable
}

/**
 * Polls the workspace HEAD SHA and invalidates only changed skills.
 *
 * Flow:
 * 1. check_workspace_head → get current SHA
 * 2. Compare with cached SHA
 * 3. If different → diff_workspace_since → get changed skill paths
 * 4. Invalidate only those skill queries in React Query
 * 5. Update cache with new SHA
 */
export function useSyncPoller({ workspace, skillPaths, focused = true, enabled = true }: SyncPollerOptions) {
  const queryClient = useQueryClient();
  const backoffRef = useRef<BackoffState>({ consecutiveNoChange: 0, lastCheckAt: 0 });
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const workspaceRef = useRef(workspace);
  const skillPathsRef = useRef(skillPaths);

  // Keep refs in sync
  workspaceRef.current = workspace;
  skillPathsRef.current = skillPaths;

  const doCheck = useCallback(async () => {
    const ws = workspaceRef.current;
    if (!ws || !isTauri) return;

    try {
      const head = await checkWorkspaceHead(ws);
      const cache = await loadWorkspaceCache(ws);
      const prevSha = cache?.headSha;

      if (!prevSha || prevSha === head.sha) {
        // No change — increase backoff
        backoffRef.current.consecutiveNoChange++;
        backoffRef.current.lastCheckAt = Date.now();

        // First time: just store the SHA
        if (!prevSha) {
          await updateCacheHead(ws, head.sha, head.branch);
        }
        return;
      }

      // SHA changed! Find which skills were affected
      const paths = skillPathsRef.current;
      if (paths.length > 0) {
        try {
          const diff = await diffWorkspaceSince({
            workspace: ws,
            baseSha: prevSha,
            headSha: head.sha,
            skillPaths: paths,
          });

          if (diff.changedSkillPaths.length > 0) {
            // Invalidate only changed skills in React Query cache
            for (const skillPath of diff.changedSkillPaths) {
              queryClient.invalidateQueries({
                queryKey: ["skill-detail", ws, skillPath],
              });
              queryClient.invalidateQueries({
                queryKey: ["demo-skill-detail", skillPath],
              });
              // Also invalidate file tree and file content queries
              queryClient.invalidateQueries({
                queryKey: ["skill-files", ws, skillPath],
              });
              queryClient.invalidateQueries({
                predicate: (query) =>
                  query.queryKey[0] === "skill-file-content" &&
                  query.queryKey[1] === ws &&
                  typeof query.queryKey[2] === "string" &&
                  (query.queryKey[2] as string).startsWith(skillPath),
              });
            }

            // Remove from IndexedDB cache (file trees + file contents)
            await invalidateSkillsInCache(ws, diff.changedSkillPaths);
          }

          // If many files changed, also refresh the workspace scan
          if (diff.totalChangedFiles > 5) {
            queryClient.invalidateQueries({
              queryKey: ["workspace-detail", ws],
            });
          }
        } catch {
          // Compare API failed (e.g. force push, too many commits)
          // Fall back to invalidating all skill queries for this workspace
          queryClient.invalidateQueries({
            predicate: (query) =>
              query.queryKey[0] === "skill-detail" && query.queryKey[1] === ws,
          });
        }
      }

      // Update stored SHA
      await updateCacheHead(ws, head.sha, head.branch);
      backoffRef.current.consecutiveNoChange = 0;
      backoffRef.current.lastCheckAt = Date.now();
    } catch {
      // Network error or auth issue — don't crash, just skip this cycle
      backoffRef.current.lastCheckAt = Date.now();
    }
  }, [queryClient]);

  // Schedule next check
  const scheduleNext = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
    }
    const interval = getInterval(backoffRef.current, focused);
    timerRef.current = setTimeout(async () => {
      await doCheck();
      scheduleNext();
    }, interval);
  }, [doCheck, focused]);

  // Start polling when workspace changes or on mount
  useEffect(() => {
    if (!enabled || !workspace || !isTauri) {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      return;
    }

    // Reset backoff when workspace changes
    backoffRef.current = { consecutiveNoChange: 0, lastCheckAt: 0 };

    // Immediate first check
    doCheck().then(() => scheduleNext());

    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [workspace, enabled, doCheck, scheduleNext]);

  // When window regains focus, do an immediate check
  useEffect(() => {
    if (!focused || !enabled || !workspace || !isTauri) return;

    const timeSinceLastCheck = Date.now() - backoffRef.current.lastCheckAt;
    // Only check if it's been more than 1 minute since last check
    if (timeSinceLastCheck > 60 * 1000) {
      doCheck().then(() => scheduleNext());
    }
  }, [focused, enabled, workspace, doCheck, scheduleNext]);
}
