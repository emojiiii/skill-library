import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Outlet, useNavigate, useRouterState } from "@tanstack/react-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useMemo, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import { useTheme } from "../hooks/useTheme";
import {
  addWorkspace,
  type DeepLinkPayload,
  exportDiagnostics,
  getAuthStatus,
  getDeepLinkState,
  installSkill,
  removeSkill,
  listProviderInstances,
  listProviderWorkspaces,
  listWorkspaces,
  loginGithubToken,
  logoutGithub,
  onDeepLink,
  openLogsFolder,
  pollGithubDeviceFlow,
  previewPublish,
  type ProviderInstance,
  type Workspace,
  startGithubDeviceFlow,
  subscribeWorkspaceSkill,
  type GitHubDeviceStartResult,
} from "../lib/skill-library";
import { LoginScreen } from "./LoginScreen";
import { Sidebar } from "./Sidebar";
import { AuthDialog } from "./AuthDialog";
import { SettingsDialog } from "./SettingsDialog";
import { AddWorkspaceDialog } from "../widgets/AddWorkspaceDialog";
import { PushModal } from "../widgets/PushModal";
import { formatError, openExternalUrl } from "../utils/format";
import { type AppPage } from "../utils/navigation";
import { useAppStore } from "../state/appStore";
import {
  workspaceInputForProvider,
  workspaceKey,
  workspaceMatchesSelection,
  workspaceProviderLabel,
} from "../lib/providers";

const defaultWorkspaceProviders: ProviderInstance[] = [
  {
    id: "github.com",
    kind: "git-hub",
    displayName: "GitHub",
    webBaseUrl: "https://github.com",
    apiBaseUrl: "https://api.github.com",
    authModes: ["personal_access_token", "device_flow"],
    enabled: true,
  },
  {
    id: "gitlab.com",
    kind: "git-lab",
    displayName: "GitLab.com",
    webBaseUrl: "https://gitlab.com",
    apiBaseUrl: "https://gitlab.com/api/v4",
    authModes: ["personal_access_token"],
    enabled: true,
  },
  {
    id: "gitee.com",
    kind: "gitee",
    displayName: "Gitee",
    webBaseUrl: "https://gitee.com",
    apiBaseUrl: "https://gitee.com/api/v5",
    authModes: ["personal_access_token"],
    enabled: true,
  },
];

/**
 * Root layout — rendered by the root route.
 * Handles: auth gate, sidebar, topbar, global modals, deep links.
 * Renders <Outlet /> for child routes (workspace pages, personal pages).
 */
export function RootLayout() {
  useTheme();
  const { t } = useLocale();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // --- Global UI state from zustand ---
  const settingsOpen = useAppStore((s) => s.settingsOpen);
  const setSettingsOpen = useAppStore((s) => s.setSettingsOpen);
  const addWorkspaceOpen = useAppStore((s) => s.addWorkspaceOpen);
  const setAddWorkspaceOpen = useAppStore((s) => s.setAddWorkspaceOpen);
  const authDialogOpen = useAppStore((s) => s.authDialogOpen);
  const setAuthDialogOpen = useAppStore((s) => s.setAuthDialogOpen);
  const pushOpen = useAppStore((s) => s.pushOpen);
  const setPushOpen = useAppStore((s) => s.setPushOpen);
  const pushEntry = useAppStore((s) => s.pushEntry);
  const setPushEntry = useAppStore((s) => s.setPushEntry);
  const pushPreview = useAppStore((s) => s.pushPreview);
  const setPushPreview = useAppStore((s) => s.setPushPreview);
  const targets = useAppStore((s) => s.targets);
  const repoQuery = useAppStore((s) => s.repoQuery);
  const setRepoQuery = useAppStore((s) => s.setRepoQuery);
  const manualPath = useAppStore((s) => s.manualPath);
  const setManualPath = useAppStore((s) => s.setManualPath);
  const githubToken = useAppStore((s) => s.githubToken);
  const setGithubToken = useAppStore((s) => s.setGithubToken);
  const dismissedError = useAppStore((s) => s.dismissedError);
  const setDismissedError = useAppStore((s) => s.setDismissedError);
  const authIntent = useAppStore((s) => s.authIntent);
  const clearAuthIntent = useAppStore((s) => s.clearAuthIntent);

  // --- Route-derived ---
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  // The active workspace is a global selection (store), not part of the URL.
  // This is the single source of truth — selecting a workspace sets it, and it
  // persists across personal pages (discover / my-skills / ...).
  const selectedWorkspace = useAppStore((s) => s.selectedWorkspace);
  const setSelectedWorkspace = useAppStore((s) => s.setSelectedWorkspace);
  const [workspaceProviderId, setWorkspaceProviderId] = useState("github.com");

  // --- Local state (auth device flow, deep links) ---
  const [githubDevice, setGithubDevice] = useState<GitHubDeviceStartResult | null>(null);
  const [githubDeviceStatus, setGithubDeviceStatus] = useState(t("device.idle"));
  const [deepLink, setDeepLink] = useState<DeepLinkPayload | null>(null);

  // --- Queries (global) ---
  const initialDeepLink = useQuery({ queryKey: ["deep-link-state"], queryFn: getDeepLinkState });
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: listWorkspaces, staleTime: 2 * 60 * 1000 });
  const auth = useQuery({ queryKey: ["auth-status"], queryFn: getAuthStatus });
  const providerInstances = useQuery({ queryKey: ["provider-instances"], queryFn: listProviderInstances, staleTime: 2 * 60 * 1000 });
  const availableProviderInstances = useMemo(
    () => (providerInstances.data?.length ? providerInstances.data : defaultWorkspaceProviders)
      .filter((provider) => provider.enabled !== false),
    [providerInstances.data],
  );
  const providerOptions = useMemo(
    () => availableProviderInstances.filter((provider) => {
      const status = auth.data?.providers?.find((entry) => entry.provider === provider.id);
      return Boolean(status?.authenticated || (provider.id === "github.com" && auth.data?.githubLogin));
    }),
    [auth.data?.githubLogin, auth.data?.providers, availableProviderInstances],
  );
  const providerOptionIds = useMemo(
    () => providerOptions.map((provider) => provider.id).join("\n"),
    [providerOptions],
  );
  useEffect(() => {
    if (!providerOptions.length) return;
    if (!providerOptions.some((provider) => provider.id === workspaceProviderId)) {
      setWorkspaceProviderId(providerOptions[0].id);
    }
  }, [providerOptionIds, providerOptions, workspaceProviderId]);
  const selectedProvider = providerOptions.find((provider) => provider.id === workspaceProviderId) ?? providerOptions[0];
  const selectedProviderStatus = auth.data?.providers?.find((provider) => provider.provider === selectedProvider?.id);
  const selectedProviderId = selectedProvider?.id ?? "";
  const selectedProviderRemoteEnabled = Boolean(selectedProviderId);
  const providerWorkspaces = useQuery({
    queryKey: ["provider-workspaces", selectedProviderId, selectedProviderStatus?.authenticated, auth.data?.githubLogin],
    queryFn: () => listProviderWorkspaces(selectedProviderId),
    enabled: Boolean(addWorkspaceOpen && selectedProviderRemoteEnabled),
    staleTime: 5 * 60 * 1000,
  });

  const workspaceMeta = workspaces.data?.workspaces.find((w) => workspaceMatchesSelection(w, selectedWorkspace)) ?? null;
  const workspaceRef = workspaceMeta ? workspaceKey(workspaceMeta) : selectedWorkspace ?? "";

  // --- Mutations ---
  // Run the pending just-in-time auth action (if any) after a successful login.
  function resumeAuthIntent() {
    const intent = useAppStore.getState().authIntent;
    intent?.resume?.();
    clearAuthIntent();
  }

  const githubLogin = useMutation({
    mutationFn: loginGithubToken,
    onSuccess: () => {
      setGithubToken("");
      auth.refetch();
      providerWorkspaces.refetch();
      setAuthDialogOpen(false);
      resumeAuthIntent();
    },
  });

  const githubLogout = useMutation({
    mutationFn: logoutGithub,
    onSuccess: () => {
      setSettingsOpen(false);
      auth.refetch();
      providerWorkspaces.refetch();
    },
  });

  const githubDeviceStart = useMutation({
    mutationFn: startGithubDeviceFlow,
    onSuccess: (device) => {
      setGithubDevice(device);
      setGithubDeviceStatus(t("device.waitingAuth"));
      const url = device.verificationUriComplete ?? device.verificationUri;
      void openExternalUrl(url);
    },
  });

  const githubDevicePoll = useMutation({
    mutationFn: pollGithubDeviceFlow,
    onSuccess: (result) => {
      if (result.status === "authorized") {
        setGithubDevice(null);
        setGithubDeviceStatus(t("device.signedInAs").replace("{login}", result.login.login));
        auth.refetch();
        providerWorkspaces.refetch();
        setAuthDialogOpen(false);
        resumeAuthIntent();
        return;
      }
      if (result.status === "slowDown") {
        setGithubDeviceStatus(t("device.slowDown").replace("{interval}", String(result.interval)));
        return;
      }
      setGithubDeviceStatus(t("device.waitingAuth"));
    },
  });

  const addRemoteWorkspace = useMutation({
    mutationFn: (workspace: Workspace) =>
      addWorkspace({ workspace: workspaceInputForProvider(workspace.provider, workspace.full_name) }),
    onSuccess: (workspace) => {
      workspaces.refetch();
      setAddWorkspaceOpen(false);
      setSelectedWorkspace(workspaceKey(workspace));
      navigate({ to: "/skills" });
    },
  });

  const addManualWorkspace = useMutation({
    mutationFn: (input: string) => addWorkspace({ workspace: workspaceInputForProvider(selectedProviderId, input) }),
    onSuccess: (workspace) => {
      workspaces.refetch();
      setAddWorkspaceOpen(false);
      setManualPath("");
      setSelectedWorkspace(workspaceKey(workspace));
      navigate({ to: "/skills" });
    },
  });

  const previewPush = useMutation({
    mutationFn: previewPublish,
    onSuccess: (preview) => setPushPreview(preview),
  });

  const confirmPush = useMutation({
    mutationFn: async () => {
      if (!pushEntry) return null;
      const target = workspaces.data?.workspaces[0]
        ? workspaceKey(workspaces.data.workspaces[0])
        : workspaceRef;
      return previewPublish({
        source: pushEntry.path,
        workspace: target,
        user: auth.data?.githubLogin ?? "local",
      });
    },
    onSuccess: () => {
      setPushOpen(false);
      setPushEntry(null);
      setPushPreview(null);
    },
  });

  const confirmDeepLink = useMutation({
    mutationFn: async () => {
      if (!deepLink) return null;
      const ws = deepLink.workspace
        ? workspaceInputForProvider(
            deepLink.workspace.provider,
            `${deepLink.workspace.owner}/${deepLink.workspace.repo}`,
          )
        : deepLink.query.workspace ?? workspaceRef;
      await subscribeWorkspaceSkill({
        workspace: ws,
        assetId: deepLink.assetId ?? "",
        version: deepLink.version ?? undefined,
        targets: deepLink.targets.length ? deepLink.targets : targets,
      });
      return ws;
    },
    onSuccess: (ws) => {
      queryClient.invalidateQueries({ queryKey: ["subscriptions"] });
      if (ws) {
        setSelectedWorkspace(ws);
        navigate({ to: "/skills" });
      }
      setDeepLink(null);
    },
  });

  const diagnostics = useMutation({ mutationFn: exportDiagnostics });
  const logsFolder = useMutation({ mutationFn: openLogsFolder });

  // --- Effects ---
  useEffect(() => {
    if (initialDeepLink.data) setDeepLink(initialDeepLink.data);
  }, [initialDeepLink.data]);

  useEffect(() => {
    if (!githubDevice) return;
    const expiresAtMs = githubDevice.expiresAt * 1000;
    const delay = Math.max(githubDevice.interval, 1) * 1000;
    const timer = window.setTimeout(() => {
      if (Date.now() >= expiresAtMs) {
        setGithubDeviceStatus(t("device.expired"));
        setGithubDevice(null);
        return;
      }
      githubDevicePoll.mutate({ clientId: githubDevice.clientId, deviceCode: githubDevice.deviceCode });
    }, delay);
    return () => window.clearTimeout(timer);
  }, [githubDevice, githubDevicePoll, githubDevicePoll.submittedAt]);

  useEffect(() => {
    let disposed = false;
    let unlisten: null | (() => void) = null;
    onDeepLink((payload) => {
      if (disposed) return;
      setDeepLink(payload);
    })
      .then((stop) => {
        if (disposed) { void stop(); return; }
        unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      if (unlisten) void unlisten();
    };
  }, []);

  // Startup landing: the consumer discover screen is the default home. We no
  // longer auto-jump into a workspace — that was a developer-centric default
  // and breaks for anonymous users who have no saved workspaces.
  useEffect(() => {
    if (pathname === "/") {
      navigate({ to: "/discover" });
    }
  }, [pathname]);

  // --- Derived ---
  const isAuthenticated = Boolean(auth.data?.githubLogin);
  const connectedProviders = auth.data?.providers?.filter((provider) => provider.authenticated) ?? [];
  const isCreatorMode = connectedProviders.length > 0 || isAuthenticated;
  const accountLabel = auth.data?.githubLogin
    ? `@${auth.data.githubLogin}`
    : connectedProviders.length
      ? t("sidebar.providerConnected").replace("{provider}", workspaceProviderLabel(connectedProviders[0].provider))
      : null;

  const globalError =
    (addRemoteWorkspace.error ? formatError(addRemoteWorkspace.error) : null) ??
    (addManualWorkspace.error ? formatError(addManualWorkspace.error) : null) ??
    (githubLogin.error ? formatError(githubLogin.error) : null) ??
    (githubLogout.error ? formatError(githubLogout.error) : null) ??
    (githubDeviceStart.error ? formatError(githubDeviceStart.error) : null) ??
    (githubDevicePoll.error ? formatError(githubDevicePoll.error) : null);
  const showGlobalError = Boolean(globalError && globalError !== dismissedError);

  const navCounts: Partial<Record<AppPage, number>> = {};

  // Open the just-in-time auth dialog whenever a gated action sets an intent.
  useEffect(() => {
    if (authIntent && !isAuthenticated) {
      setAuthDialogOpen(true);
    }
  }, [authIntent, isAuthenticated]);

  // --- Window drag handler ---
  // Allow dragging the window from the top ~40px of the main area,
  // but only when clicking on empty space (not interactive elements).
  const handleDragMouseDown = (e: React.MouseEvent) => {
    // Only trigger in the top titlebar region
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const y = e.clientY - rect.top;
    if (y > 40) return;

    // Don't interfere with interactive elements
    const target = e.target as HTMLElement;
    const interactive = target.closest("button, a, input, select, textarea, [role='button'], [data-no-drag]");
    if (interactive) return;

    e.preventDefault();
    getCurrentWindow().startDragging();
  };

  // --- Loading state ---
  if (auth.isLoading) {
    return (
      <div className="grid h-screen place-items-center bg-[var(--bg)]">
        <div className="text-[13px] text-[var(--fg-muted)]">{t("common.loading")}</div>
      </div>
    );
  }

  // --- Login screen ---
  // No hard login wall: the consumer layer (discover / my skills) is fully
  // anonymous. GitHub sign-in is requested just-in-time via AuthDialog when a
  // gated action (publish, comment, manage) needs it. `LoginScreen` is kept
  // importable for the optional first-run welcome but is no longer a gate.
  void LoginScreen;

  return (
    <div className="app-shell">
      <Sidebar
        current={
          workspaceMeta
            ? workspaceMeta
            : workspaceRef
              ? {
                  provider: workspaceProviderId,
                  full_name: workspaceRef,
                  permission: "—",
                  visibility: "public",
                }
              : null
        }
        saved={workspaces.data?.workspaces ?? []}
        onSelectWorkspace={(workspace) => {
          setSelectedWorkspace(workspaceKey(workspace));
          navigate({ to: "/skills" });
        }}
        onOpenAddDialog={() => setAddWorkspaceOpen(true)}
        counts={navCounts}
        authLogin={accountLabel}
        isCreatorMode={isCreatorMode}
        onOpenAccount={() => setSettingsOpen(true)}
      />

      <main className="app-shell__main" onMouseDown={handleDragMouseDown}>
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        {deepLink ? (
          <div className="banner banner--accent">
            <div className="min-w-0">
              <div className="text-[13px] font-medium">{t("deepLink.received")}</div>
              <div className="truncate text-[11.5px] opacity-80">{deepLink.action} · {deepLink.url}</div>
            </div>
            <div className="flex gap-2">
              <button
                type="button"
                className="rounded-md border border-[var(--brand)] bg-[var(--brand-soft)] px-3 py-1 text-[12px] font-medium text-[var(--brand-fg)]"
                onClick={() => confirmDeepLink.mutate()}
              >
                {t("deepLink.subscribe")}
              </button>
              <button
                type="button"
                className="rounded-md px-2 py-1 text-[12px] text-[var(--brand-fg)] hover:bg-white/40"
                onClick={() => setDeepLink(null)}
              >
                {t("deepLink.dismiss")}
              </button>
            </div>
          </div>
        ) : null}

        {showGlobalError ? (
          <div className="banner banner--danger">
            <div className="min-w-0">
              <div className="text-[13px] font-medium">{t("error.requestFailed")}</div>
              <div className="truncate text-[11.5px] opacity-80">{globalError}</div>
            </div>
            <button
              type="button"
              className="rounded-md px-2 py-1 text-[12px] text-[var(--danger)] hover:bg-white/40"
              onClick={() => setDismissedError(globalError ?? "")}
            >
              {t("deepLink.dismiss")}
            </button>
          </div>
        ) : null}

        <div className="flex min-h-0 flex-1 flex-col">
          <Outlet />
        </div>
        </div>
      </main>

      {/* Global modals */}
      <AddWorkspaceDialog
        open={addWorkspaceOpen}
        onOpenChange={setAddWorkspaceOpen}
        remote={providerWorkspaces.data ?? []}
        remoteFetching={providerWorkspaces.isFetching}
        remoteEnabled={selectedProviderRemoteEnabled}
        providers={providerOptions}
        selectedProviderId={selectedProviderId}
        onProviderChange={(providerId) => {
          setWorkspaceProviderId(providerId);
          setRepoQuery("");
          setManualPath("");
        }}
        query={repoQuery}
        setQuery={setRepoQuery}
        onAddRemote={(workspace) => addRemoteWorkspace.mutate(workspace)}
        isAddingFullName={addRemoteWorkspace.variables ? workspaceKey(addRemoteWorkspace.variables) : undefined}
        manualPath={manualPath}
        setManualPath={setManualPath}
        onAddManual={() => addManualWorkspace.mutate(manualPath.trim())}
        manualPending={addManualWorkspace.isPending}
      />

      <PushModal
        open={pushOpen}
        onOpenChange={(value) => {
          setPushOpen(value);
          if (!value) setPushPreview(null);
        }}
        entry={pushEntry}
        workspaces={workspaces.data?.workspaces ?? []}
        preview={pushPreview}
        previewPending={previewPush.isPending}
        onPreview={(input) => previewPush.mutate(input)}
        onConfirm={() => confirmPush.mutate()}
        confirmPending={confirmPush.isPending}
      />

      <AuthDialog
        open={authDialogOpen}
        onOpenChange={(value) => {
          setAuthDialogOpen(value);
          if (!value) clearAuthIntent();
        }}
        intentReason={authIntent ? t(`auth.reason.${authIntent.action}`) : undefined}
        authLogin={auth.data?.githubLogin}
        authScopes={auth.data?.githubScopes ?? []}
        authWarning={auth.data?.warning}
        onStartDevice={() => githubDeviceStart.mutate(undefined)}
        startPending={githubDeviceStart.isPending}
        startError={githubDeviceStart.error ? formatError(githubDeviceStart.error) : null}
        device={githubDevice}
        deviceStatus={githubDeviceStatus}
        pollPending={githubDevicePoll.isPending}
        pollError={githubDevicePoll.error ? formatError(githubDevicePoll.error) : null}
        githubToken={githubToken}
        setGithubToken={setGithubToken}
        onSaveToken={() => githubLogin.mutate(githubToken)}
        savePending={githubLogin.isPending}
        saveError={githubLogin.error ? formatError(githubLogin.error) : null}
      />

      <SettingsDialog
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
        authLogin={auth.data?.githubLogin}
        authScopes={auth.data?.githubScopes ?? []}
        onLogin={() => {
          clearAuthIntent();
          setSettingsOpen(false);
          setAuthDialogOpen(true);
        }}
        logoutPending={githubLogout.isPending}
        onLogout={() => githubLogout.mutate()}
      />
    </div>
  );
}
