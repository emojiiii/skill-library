import { create } from "zustand";
import type { LocalAgentEntry } from "../lib/teamai";
import type { PublishPreview } from "../lib/teamai";
import type { InviteRole } from "../utils/navigation";

/**
 * Global UI state — persists across workspace switches.
 * Workspace-scoped state lives in WorkspaceShell (remounts on switch).
 */
interface AppState {
  // --- Dialogs ---
  settingsOpen: boolean;
  setSettingsOpen: (v: boolean) => void;
  addWorkspaceOpen: boolean;
  setAddWorkspaceOpen: (v: boolean) => void;
  authDialogOpen: boolean;
  setAuthDialogOpen: (v: boolean) => void;
  pushOpen: boolean;
  setPushOpen: (v: boolean) => void;
  pushEntry: LocalAgentEntry | null;
  setPushEntry: (v: LocalAgentEntry | null) => void;
  pushPreview: PublishPreview | null;
  setPushPreview: (v: PublishPreview | null) => void;

  // --- Install targets (persists across workspaces) ---
  targets: string[];
  setTargets: (v: string[]) => void;

  // --- Add workspace form ---
  repoQuery: string;
  setRepoQuery: (v: string) => void;
  manualPath: string;
  setManualPath: (v: string) => void;

  // --- GitHub device flow ---
  githubToken: string;
  setGithubToken: (v: string) => void;

  // --- Invite ---
  inviteLogin: string;
  setInviteLogin: (v: string) => void;
  inviteRole: InviteRole;
  setInviteRole: (v: InviteRole) => void;

  // --- Error banner ---
  dismissedError: string;
  setDismissedError: (v: string) => void;
}

export const useAppStore = create<AppState>((set) => ({
  settingsOpen: false,
  setSettingsOpen: (v) => set({ settingsOpen: v }),
  addWorkspaceOpen: false,
  setAddWorkspaceOpen: (v) => set({ addWorkspaceOpen: v }),
  authDialogOpen: false,
  setAuthDialogOpen: (v) => set({ authDialogOpen: v }),
  pushOpen: false,
  setPushOpen: (v) => set({ pushOpen: v }),
  pushEntry: null,
  setPushEntry: (v) => set({ pushEntry: v }),
  pushPreview: null,
  setPushPreview: (v) => set({ pushPreview: v }),

  targets: ["claude-code", "cursor", "codex"],
  setTargets: (v) => set({ targets: v }),

  repoQuery: "",
  setRepoQuery: (v) => set({ repoQuery: v }),
  manualPath: "",
  setManualPath: (v) => set({ manualPath: v }),

  githubToken: "",
  setGithubToken: (v) => set({ githubToken: v }),

  inviteLogin: "",
  setInviteLogin: (v) => set({ inviteLogin: v }),
  inviteRole: "read",
  setInviteRole: (v) => set({ inviteRole: v }),

  dismissedError: "",
  setDismissedError: (v) => set({ dismissedError: v }),
}));
