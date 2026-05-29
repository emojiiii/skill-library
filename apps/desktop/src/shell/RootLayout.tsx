import { useMutation, useQuery } from "@tanstack/react-query";
import { Outlet, useNavigate, useRouterState } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import { useTheme } from "../hooks/useTheme";
import {
  addWorkspace,
  appInit,
  type DeepLinkPayload,
  exportDiagnostics,
  getAuthStatus,
  getDeepLinkState,
  installSkill,
  removeSkill,
  listGithubWorkspaces,
  listLocalAgentRoots,
  listWorkspaces,
  loginGithubToken,
  onDeepLink,
  openLogsFolder,
  pollGithubDeviceFlow,
  previewPublish,
  readSubscriptions,
  type Workspace,
  startGithubDeviceFlow,
  subscribeWorkspaceSkill,
  type GitHubDeviceStartResult,
} from "../lib/teamai";
import { LoginScreen } from "./LoginScreen";
import { Sidebar } from "./Sidebar";
import { Topbar } from "./Topbar";
import { AuthDialog } from "./AuthDialog";
import { SettingsDialog } from "./SettingsDialog";
import { AddWorkspaceDialog } from "../widgets/AddWorkspaceDialog";
import { PushModal } from "../widgets/PushModal";
import { formatError, openExternalUrl } from "../utils/format";
import { type AppPage, routeToPage, workspaceFromPathname } from "../utils/navigation";
import { useAppStore } from "../state/appStore";

/**
 * Root layout — rendered by the root route.
 * Handles: auth gate, sidebar, topbar, global modals, deep links.
 * Renders <Outlet /> for child routes (workspace pages, personal pages).
 */
export function RootLayout() {
  useTheme();
  const { t } = useLocale();
  const navigate = useNavigate();

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

  // --- Route-derived ---
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const currentPage = routeToPage(pathname);
  const workspaceRef = workspaceFromPathname(pathname) ?? "";

  // --- Local state (auth device flow, deep links) ---
  const [githubDevice, setGithubDevice] = useState<GitHubDeviceStartResult | null>(null);
  const [githubDeviceStatus, setGithubDeviceStatus] = useState(t("device.idle"));
  const [deepLink, setDeepLink] = useState<DeepLinkPayload | null>(null);

  // --- Queries (global) ---
  useQuery({ queryKey: ["init"], queryFn: appInit });
  const initialDeepLink = useQuery({ queryKey: ["deep-link-state"], queryFn: getDeepLinkState });
  const subscriptions = useQuery({ queryKey: ["subscriptions"], queryFn: readSubscriptions, staleTime: 60 * 1000 });
  const localAgents = useQuery({ queryKey: ["local-agents"], queryFn: listLocalAgentRoots, staleTime: 60 * 1000 });
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: listWorkspaces, staleTime: 2 * 60 * 1000 });
  const auth = useQuery({ queryKey: ["auth-status"], queryFn: getAuthStatus });
  const githubRepos = useQuery({
    queryKey: ["github-workspaces", auth.data?.githubLogin],
    queryFn: () => listGithubWorkspaces(),
    enabled: Boolean(auth.data?.githubLogin),
    staleTime: 5 * 60 * 1000,
  });

  const workspaceMeta = workspaces.data?.workspaces.find((w) => w.full_name === workspaceRef) ?? null;

  // --- Mutations ---
  const githubLogin = useMutation({
    mutationFn: loginGithubToken,
    onSuccess: () => {
      setGithubToken("");
      auth.refetch();
      githubRepos.refetch();
      setAuthDialogOpen(false);
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
        githubRepos.refetch();
        setAuthDialogOpen(false);
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
    mutationFn: (workspace: Workspace) => addWorkspace({ workspace: workspace.full_name }),
    onSuccess: (workspace) => {
      workspaces.refetch();
      setAddWorkspaceOpen(false);
      const [owner, repo] = workspace.full_name.split("/");
      navigate({ to: "/workspace/$owner/$repo", params: { owner, repo } });
    },
  });

  const addManualWorkspace = useMutation({
    mutationFn: (input: string) => addWorkspace({ workspace: input }),
    onSuccess: (workspace) => {
      workspaces.refetch();
      setAddWorkspaceOpen(false);
      setManualPath("");
      const [owner, repo] = workspace.full_name.split("/");
      navigate({ to: "/workspace/$owner/$repo", params: { owner, repo } });
    },
  });

  const previewPush = useMutation({
    mutationFn: previewPublish,
    onSuccess: (preview) => setPushPreview(preview),
  });

  const confirmPush = useMutation({
    mutationFn: async () => {
      if (!pushEntry) return null;
      const target = workspaces.data?.workspaces[0]?.full_name ?? workspaceRef;
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
        ? `${deepLink.workspace.owner}/${deepLink.workspace.repo}`
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
      subscriptions.refetch();
      if (ws) {
        const [owner, repo] = ws.split("/");
        navigate({ to: "/workspace/$owner/$repo", params: { owner, repo } });
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

  // Auto-navigate to first workspace on startup (when at index route)
  useEffect(() => {
    if (!workspaceRef && pathname === "/" && workspaces.data?.workspaces.length) {
      const first = workspaces.data.workspaces[0];
      const [owner, repo] = first.full_name.split("/");
      navigate({ to: "/workspace/$owner/$repo", params: { owner, repo } });
    }
  }, [workspaces.data?.workspaces, workspaceRef, pathname]);

  // --- Derived ---
  const isAuthenticated = Boolean(auth.data?.githubLogin);

  const refreshControlPlane = () => {
    workspaces.refetch();
    localAgents.refetch();
    subscriptions.refetch();
  };
  const controlPlanePending = workspaces.isFetching || localAgents.isFetching || subscriptions.isFetching;

  const globalError =
    (addRemoteWorkspace.error ? formatError(addRemoteWorkspace.error) : null) ??
    (addManualWorkspace.error ? formatError(addManualWorkspace.error) : null) ??
    (githubLogin.error ? formatError(githubLogin.error) : null) ??
    (githubDeviceStart.error ? formatError(githubDeviceStart.error) : null) ??
    (githubDevicePoll.error ? formatError(githubDevicePoll.error) : null);
  const showGlobalError = Boolean(globalError && globalError !== dismissedError);

  const navCounts: Partial<Record<AppPage, number>> = {
    subscriptions: subscriptions.data?.subscriptions.length,
    installed: localAgents.data
      ? new Set(localAgents.data.flatMap((g) => g.entries.map((e) => e.id))).size
      : undefined,
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
  if (!isAuthenticated) {
    return (
      <LoginScreen
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
        authScopes={auth.data?.githubScopes ?? []}
        authWarning={auth.data?.warning}
      />
    );
  }

  return (
    <div className="app-shell">
      <Sidebar
        current={
          workspaceRef
            ? {
                full_name: workspaceRef,
                permission: workspaceMeta?.permission ?? "—",
                visibility: workspaceMeta?.visibility ?? "public",
              }
            : null
        }
        saved={workspaces.data?.workspaces ?? []}
        onSelectWorkspace={(workspace) => {
          const [owner, repo] = workspace.full_name.split("/");
          navigate({ to: "/workspace/$owner/$repo", params: { owner, repo } });
        }}
        onOpenAddDialog={() => setAddWorkspaceOpen(true)}
        counts={navCounts}
        authLogin={auth.data?.githubLogin}
        onOpenAccount={() => setSettingsOpen(true)}
      />

      <main className="app-shell__main">
        <Topbar
          page={currentPage}
          onRefresh={refreshControlPlane}
          refreshing={controlPlanePending}
        />

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
      </main>

      {/* Global modals */}
      <AddWorkspaceDialog
        open={addWorkspaceOpen}
        onOpenChange={setAddWorkspaceOpen}
        remote={githubRepos.data ?? []}
        remoteFetching={githubRepos.isFetching}
        remoteEnabled={isAuthenticated}
        query={repoQuery}
        setQuery={setRepoQuery}
        onAddRemote={(workspace) => addRemoteWorkspace.mutate(workspace)}
        isAddingFullName={addRemoteWorkspace.variables?.full_name}
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
        onOpenChange={setAuthDialogOpen}
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
        onLogout={() => {
          setSettingsOpen(false);
          auth.refetch();
        }}
      />
    </div>
  );
}
