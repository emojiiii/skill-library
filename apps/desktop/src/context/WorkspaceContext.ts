import { createContext, useContext } from "react";
import type { StoredWorkspace } from "../lib/skill-library";

export interface WorkspaceContextValue {
  /** e.g. "owner/repo" */
  workspace: string;
  /** Metadata from the workspaces list, null if not found */
  workspaceMeta: StoredWorkspace | null;
  /** Authenticated GitHub login */
  authLogin: string | null | undefined;
}

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null);

export const WorkspaceProvider = WorkspaceContext.Provider;

export function useWorkspace(): WorkspaceContextValue {
  const ctx = useContext(WorkspaceContext);
  if (!ctx) throw new Error("useWorkspace must be used within WorkspaceProvider (a workspace route)");
  return ctx;
}
