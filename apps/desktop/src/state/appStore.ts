import { create } from "zustand";
import type { LocalAgentEntry } from "../lib/teamai";
import type { PublishPreview } from "../lib/teamai";
import type { InviteRole } from "../utils/navigation";

const SELECTED_WS_KEY = "teamai-selected-workspace";

function readStoredWorkspace(): string | null {
  try {
    return localStorage.getItem(SELECTED_WS_KEY);
  } catch {
    return null;
  }
}

/**
 * What the user was trying to do when an action required GitHub login.
 * After a just-in-time sign-in succeeds, we run `resume()` to continue.
 */
export type AuthIntentAction = "comment" | "publish" | "invite" | "browsePrivate" | "manage";

export interface AuthIntent {
  action: AuthIntentAction;
  /** Optional callback fired once login completes. */
  resume?: () => void;
}

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

  // --- Just-in-time auth (consumer layer stays anonymous until a gated action) ---
  authIntent: AuthIntent | null;
  /** Open the just-in-time login prompt for a gated action. */
  requestAuth: (intent: AuthIntent) => void;
  /** Clear the pending intent (cancel or after resume). */
  clearAuthIntent: () => void;

  // --- Install targets (persists across workspaces) ---
  targets: string[];
  setTargets: (v: string[]) => void;

  // --- Selected workspace (sticky across personal pages) ---
  /**
   * The workspace the user last selected. Stays set when visiting personal
   * pages (discover / my-skills / subscriptions / installed / cli) so the
   * sidebar picker and workspace nav links don't reset. Workspace routes sync
   * this from the URL; personal routes leave it untouched.
   */
  selectedWorkspace: string | null;
  setSelectedWorkspace: (v: string | null) => void;

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

  authIntent: null,
  requestAuth: (intent) => set({ authIntent: intent }),
  clearAuthIntent: () => set({ authIntent: null }),

  // Tools default to OFF. An empty selection means "download locally without
  // deploying to any tool"; the backend treats explicit-empty as download-only
  // rather than installing everywhere.
  targets: [],
  setTargets: (v) => set({ targets: v }),

  selectedWorkspace: readStoredWorkspace(),
  setSelectedWorkspace: (v) => {
    try {
      if (v) localStorage.setItem(SELECTED_WS_KEY, v);
      else localStorage.removeItem(SELECTED_WS_KEY);
    } catch { /* ignore */ }
    set({ selectedWorkspace: v });
  },

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
