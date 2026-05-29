import { Outlet, useParams } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { getAuthStatus, listWorkspaces } from "../lib/teamai";
import { WorkspaceProvider, type WorkspaceContextValue } from "../context/WorkspaceContext";

/**
 * Workspace layout route component.
 *
 * Mounted at /workspace/$owner/$repo — provides workspace context to child routes.
 * When the user navigates to a different workspace, params change → queries re-key → children re-render with fresh data.
 */
export function WorkspaceShell() {
  const { owner, repo } = useParams({ from: "/workspace/$owner/$repo" });
  const workspace = `${owner}/${repo}`;

  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: listWorkspaces, staleTime: 2 * 60 * 1000 });
  const auth = useQuery({ queryKey: ["auth-status"], queryFn: getAuthStatus });

  const workspaceMeta = useMemo(
    () => workspaces.data?.workspaces.find((w) => w.full_name === workspace) ?? null,
    [workspaces.data?.workspaces, workspace],
  );

  const ctx: WorkspaceContextValue = useMemo(
    () => ({ workspace, workspaceMeta, authLogin: auth.data?.githubLogin }),
    [workspace, workspaceMeta, auth.data?.githubLogin],
  );

  return (
    <WorkspaceProvider value={ctx}>
      <Outlet />
    </WorkspaceProvider>
  );
}
