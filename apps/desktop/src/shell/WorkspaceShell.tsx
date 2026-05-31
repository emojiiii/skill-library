import { Outlet } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { Button } from "@heroui/react";
import { PackageOpen } from "lucide-react";
import { getAuthStatus, listWorkspaces } from "../lib/teamai";
import { WorkspaceProvider, type WorkspaceContextValue } from "../context/WorkspaceContext";
import { useAppStore } from "../state/appStore";
import { useLocale } from "../hooks/useLocale";

/**
 * Workspace layout route component (pathless layout route).
 *
 * The active workspace is a GLOBAL selection (appStore.selectedWorkspace), not
 * part of the URL — this is a desktop app, so there's no benefit to encoding it
 * in the path. Provides workspace context to the four workspace-scoped pages
 * (/skills, /publish, /members, /activity).
 *
 * Children are keyed by the selected workspace so switching workspaces remounts
 * them with fresh state (equivalent to the old `key={owner/repo}` on the index
 * route). When no workspace is selected, an empty state is shown instead.
 */
export function WorkspaceShell() {
  const { t } = useLocale();
  const workspace = useAppStore((s) => s.selectedWorkspace);
  const setAddWorkspaceOpen = useAppStore((s) => s.setAddWorkspaceOpen);

  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: listWorkspaces, staleTime: 2 * 60 * 1000 });
  const auth = useQuery({ queryKey: ["auth-status"], queryFn: getAuthStatus });

  const workspaceMeta = useMemo(
    () => workspaces.data?.workspaces.find((w) => w.full_name === workspace) ?? null,
    [workspaces.data?.workspaces, workspace],
  );

  const ctx: WorkspaceContextValue = useMemo(
    () => ({ workspace: workspace ?? "", workspaceMeta, authLogin: auth.data?.githubLogin }),
    [workspace, workspaceMeta, auth.data?.githubLogin],
  );

  if (!workspace) {
    return (
      <div className="grid flex-1 place-items-center p-8">
        <div className="flex max-w-sm flex-col items-center gap-3 text-center">
          <span className="grid size-12 place-items-center rounded-full bg-[var(--bg-soft)] text-[var(--fg-muted)]">
            <PackageOpen size={22} />
          </span>
          <div>
            <div className="text-[15px] font-semibold text-[var(--fg)]">
              {t("workspaceShell.noSelection")}
            </div>
            <div className="mt-1 text-[13px] text-[var(--fg-muted)]">
              {t("workspaceShell.noSelection.desc")}
            </div>
          </div>
          <Button size="sm" variant="secondary" onPress={() => setAddWorkspaceOpen(true)}>
            {t("workspaceShell.addWorkspace")}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <WorkspaceProvider value={ctx}>
      <Outlet key={workspace} />
    </WorkspaceProvider>
  );
}
