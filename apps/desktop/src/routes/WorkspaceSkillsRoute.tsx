import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Spinner } from "@heroui/react";
import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { useSyncPoller } from "../hooks/useSyncPoller";
import { useWindowFocus } from "../hooks/useWindowFocus";
import { useLocalStorage } from "../hooks/useLocalStorage";
import { getSkillsListCache, setSkillsListCache, getSkillDetailFromCache, putSkillDetailInCache, clearSkillDetail, getReviewCache, getReviewCaches, putReviewCache } from "../lib/workspaceCache";
import {
  getSkillDetail,
  getRemoteReviewsBatch,
  getWorkspaceDetail,
  installSkill,
  onScanProgress,
  previewPublish,
  scanGithubWorkspaceStreaming,
  scanWorkspace,
  type SkillAsset,
  subscribeWorkspaceSkill,
  type Workspace,
  type WorkspaceDetail,
  listWorkspaces,
} from "../lib/teamai";
import { normalizeRemoteReview, reviewVerdictMapKey, type ReviewVerdictMap } from "../lib/review";
import { WorkspacesPage } from "../pages/WorkspacesPage";
import { PublishModal } from "../widgets/PublishModal";
import { SubscribeModal, type Channel, type UpdatePolicy } from "../widgets/SubscribeModal";
import { SyncSkillModal } from "../widgets/SyncSkillModal";
import { useWorkspace } from "../context/WorkspaceContext";
import { useAppStore } from "../state/appStore";

// Lazy-loaded: SkillDetail pulls in the heavy editor stack (MDXEditor/ProseMirror
// + CodeMirror with 10 language packs, ~2MB). Keeping it out of the startup
// bundle is what stops the multi-second main-thread stall on first paint — the
// editor chunk is only fetched when the detail modal is actually opened.
const SkillDetail = lazy(() => import("../widgets/SkillDetail").then((m) => ({ default: m.SkillDetail })));

type ScanRequest = { kind: "local" | "github"; value: string };

function resolveSelectedSource(workspace: string, selected: SkillAsset | null) {
  if (!selected || workspace === "demo") return "demo";
  return `${workspace.replace(/\/$/, "")}/${selected.path}`;
}

export function WorkspaceSkillsRoute() {
  const { workspace, workspaceMeta, authLogin } = useWorkspace();
  const queryClient = useQueryClient();
  const windowFocused = useWindowFocus();
  const targets = useAppStore((s) => s.targets);
  const setTargets = useAppStore((s) => s.setTargets);

  // Persisted per-workspace UI state via localStorage
  const [persistedSkillId, setPersistedSkillId] = useLocalStorage<string | null>(`ws-ui:${workspace}:skillId`, null);
  const [persistedFile, setPersistedFile] = useLocalStorage<string | null>(`ws-ui:${workspace}:file`, null);

  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: listWorkspaces, staleTime: 2 * 60 * 1000 });
  const subscriptions = useQuery({ queryKey: ["subscriptions"], queryFn: () => import("../lib/teamai").then((m) => m.readSubscriptions()), staleTime: 60 * 1000 });

  // --- Local state ---
  const [selected, setSelectedRaw] = useState<SkillAsset | null>(null);
  const [selectedRef, setSelectedRef] = useState<string | undefined>();
  const [selectedFile, setSelectedFileRaw] = useState<string | null>(persistedFile);
  const [query, setQuery] = useState("");
  const [detailOpen, setDetailOpen] = useState(false);
  const [subscribeOpen, setSubscribeOpen] = useState(false);
  const [syncOpen, setSyncOpen] = useState(false);
  const [publishOpen, setPublishOpen] = useState(false);
  const [demoWorkspaceDetail, setDemoWorkspaceDetail] = useState<WorkspaceDetail | null>(null);
  const [cachedSkills, setCachedSkills] = useState<{ workspace: string; skills: SkillAsset[] }>({ workspace: "", skills: [] });
  const [streamingSkills, setStreamingSkills] = useState<SkillAsset[]>([]);
  const prevAssetsRef = useRef<SkillAsset[]>([]);
  const initialLoadDone = useRef(false);
  const unlistenRef = useRef<(() => void) | null>(null);

  // Simple setters that also persist
  const setSelected = (asset: SkillAsset | null) => {
    setSelectedRaw(asset);
    if (asset) setPersistedSkillId(asset.manifest.id);
  };

  const setSelectedFile = (file: string | null) => {
    setSelectedFileRaw(file);
    setPersistedFile(file);
  };

  // --- Scan mutation (streaming) ---
  const scan = useMutation({
    mutationFn: async (request: ScanRequest) => {
      // Reset streaming state and set up listener before starting scan
      setStreamingSkills([]);
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
      // Register streaming listener
      const unlisten = await onScanProgress((batch) => {
        setStreamingSkills((prev) => [...prev, ...batch]);
      });
      unlistenRef.current = unlisten;

      try {
        if (request.kind === "local" && request.value.trim() === "demo") {
          const detail = await getWorkspaceDetail({ workspace: request.value });
          return { workspace: null, skills: await scanWorkspace("demo"), detail, fromCache: false };
        }
        if (request.kind === "local") {
          return { workspace: null, skills: await scanWorkspace(request.value), detail: null, fromCache: false };
        }
        try {
          const detail = await getWorkspaceDetail({ workspace: request.value });
          return { workspace: detail.workspace, skills: detail.skills, detail, fromCache: false };
        } catch {
          const fallback = await scanGithubWorkspaceStreaming({ workspace: request.value });
          return { workspace: fallback.workspace, skills: fallback.skills, detail: null, fromCache: false };
        }
      } finally {
        // Clean up listener when scan completes (success or error)
        if (unlistenRef.current) {
          unlistenRef.current();
          unlistenRef.current = null;
        }
      }
    },
    onSuccess: (result) => {
      setStreamingSkills([]); // Clear streaming state, final result is authoritative
      setDemoWorkspaceDetail(result.workspace ? null : result.detail);
      // Restore persisted skill or fall back to first
      const restored = persistedSkillId
        ? result.skills.find((s) => s.manifest.id === persistedSkillId)
        : null;
      if (restored) {
        setSelectedRaw(restored);
      } else if (!selected || !result.skills.some((s) => s.manifest.id === selected.manifest.id)) {
        setSelected(result.skills[0] ?? null);
      }
      setSelectedRef(undefined);
      const wsName = result.workspace?.full_name ?? workspace;
      if (wsName) {
        queryClient.setQueryData(["workspace-scan", wsName], result);
        void setSkillsListCache(wsName, result.skills);
      }
    },
  });

  // Clean up listener on unmount
  useEffect(() => {
    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    };
  }, []);

  // --- Initial load ---
  if (!initialLoadDone.current) {
    initialLoadDone.current = true;
    const cachedScan = queryClient.getQueryData<{ workspace: Workspace | null; skills: SkillAsset[]; detail: WorkspaceDetail | null }>(
      ["workspace-scan", workspace]
    );
    if (cachedScan) {
      prevAssetsRef.current = cachedScan.skills;
      if (!selected && cachedScan.skills.length > 0) {
        const restored = persistedSkillId
          ? cachedScan.skills.find((s) => s.manifest.id === persistedSkillId)
          : null;
        queueMicrotask(() => setSelectedRaw(restored ?? cachedScan.skills[0] ?? null));
      }
    } else {
      void getSkillsListCache(workspace).then((cached) => {
        if (cached && cached.length > 0) {
          const skills = cached as SkillAsset[];
          setCachedSkills({ workspace, skills });
          setSelectedRaw((prev) => {
            if (prev) return prev;
            const restored = persistedSkillId
              ? skills.find((s) => s.manifest.id === persistedSkillId)
              : null;
            return restored ?? skills[0] ?? null;
          });
        }
      });
    }
    scan.mutate({ kind: "github", value: workspace });
  }

  // --- Skill detail ---
  const skillDetail = useQuery({
    queryKey: ["skill-detail", workspace, selected?.path, selectedRef],
    queryFn: async () => {
      const skillPath = selected?.path ?? "";
      // Render instantly from cache, then refresh in the background.
      const cached = await getSkillDetailFromCache<Awaited<ReturnType<typeof getSkillDetail>>>(
        workspace,
        skillPath,
        selectedRef,
      );
      if (cached) {
        void getSkillDetail({ workspace, skillPath, refName: selectedRef })
          .then((fresh) => putSkillDetailInCache(workspace, skillPath, selectedRef, fresh))
          .catch(() => undefined);
        return cached;
      }
      const fresh = await getSkillDetail({ workspace, skillPath, refName: selectedRef });
      await putSkillDetailInCache(workspace, skillPath, selectedRef, fresh);
      return fresh;
    },
    enabled: Boolean(workspaceMeta && selected?.path),
    staleTime: 2 * 60 * 1000,
  });

  const demoSkillDetail = useQuery({
    queryKey: ["demo-skill-detail", selected?.path, selectedRef],
    queryFn: () => getSkillDetail({ workspace, skillPath: selected?.path ?? "", refName: selectedRef }),
    enabled: Boolean(!workspaceMeta && selected?.path),
    staleTime: 2 * 60 * 1000,
  });

  const selectedDetail = workspaceMeta ? skillDetail : demoSkillDetail;
  const workspaceDetail = workspaceMeta ? scan.data?.detail ?? null : demoWorkspaceDetail;

  // --- Mutations ---
  const subscribe = useMutation({
    mutationFn: (input: { targets: string[]; policy: UpdatePolicy; channel: Channel }) =>
      subscribeWorkspaceSkill({
        workspace,
        assetId: selected?.manifest.id ?? "",
        version: selected?.manifest.version,
        targets: input.targets,
      }),
    onSuccess: () => {
      subscriptions.refetch();
      setSubscribeOpen(false);
    },
  });

  const install = useMutation<Awaited<ReturnType<typeof installSkill>>, Error, boolean | undefined>({
    mutationFn: (confirmed = false) =>
      installSkill(resolveSelectedSource(workspace, selected), targets, confirmed),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["local-agents"] }),
  });

  const publish = useMutation({
    mutationFn: () =>
      previewPublish({ source: resolveSelectedSource(workspace, selected), workspace, user: "local" }),
  });

  // --- Derived: assets with filter ---
  const currentAssets = scan.data?.skills ?? [];
  if (currentAssets.length > 0) prevAssetsRef.current = currentAssets;
  // During streaming, show incrementally discovered skills; once scan completes, use final result
  const assets = currentAssets.length > 0
    ? currentAssets
    : streamingSkills.length > 0
      ? streamingSkills
      : (prevAssetsRef.current.length > 0 ? prevAssetsRef.current : (cachedSkills.workspace === workspace ? cachedSkills.skills : []));

  const filteredAssets = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return assets;
    return assets.filter((asset) => {
      const haystack = [asset.manifest.id, asset.manifest.name, asset.manifest.description, ...asset.manifest.tags]
        .join(" ").toLowerCase();
      return haystack.includes(needle);
    });
  }, [assets, query]);

  // --- Sync poller ---
  const skillPaths = useMemo(() => assets.map((a) => a.path), [assets]);
  useSyncPoller({ workspace: workspace || null, skillPaths, focused: windowFocused, enabled: Boolean(workspace) });

  // --- Review verdict map: SQLite first, remote refresh second ---
  // The review for a skill is just a JSON file in the repo's `.reviews/` dir. We
  // rebuild the badge map from SQLite immediately, then batch-fetch remote
  // reviews silently to refresh that local cache in the background.
  const reviewTargets = useMemo(
    () => assets.map((asset) => ({ id: asset.manifest.id, path: asset.path })),
    [assets],
  );
  const skillIds = useMemo(() => reviewTargets.map((target) => target.id), [reviewTargets]);
  const skillIdsKey = useMemo(() => skillIds.join("\0"), [skillIds]);

  useEffect(() => {
    const key = reviewVerdictMapKey(workspace);
    if (!workspace || !reviewTargets.length) {
      queryClient.setQueryData(key, {});
      return;
    }

    let cancelled = false;
    void getReviewCaches(workspace, reviewTargets.map((target) => target.path))
      .then((entries) => {
        if (cancelled) return;
        const localMap: ReviewVerdictMap = {};
        reviewTargets.forEach((target, index) => {
          const entry = entries[index];
          if (!entry) return;
          localMap[target.id] = entry.verdict;
          queryClient.setQueryData(["review-cache", workspace, target.path], entry);
        });
        queryClient.setQueryData<ReviewVerdictMap>(key, (current) => {
          const next: ReviewVerdictMap = {};
          for (const target of reviewTargets) {
            const verdict = localMap[target.id] ?? current?.[target.id];
            if (verdict) next[target.id] = verdict;
          }
          return next;
        });
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, [workspace, reviewTargets, queryClient]);

  useEffect(() => {
    if (!workspace || !authLogin || !skillIds.length) return;
    void queryClient.invalidateQueries({ queryKey: reviewVerdictMapKey(workspace) });
  }, [workspace, authLogin, skillIds.length, skillIdsKey, queryClient]);

  const verdictMapQuery = useQuery({
    queryKey: reviewVerdictMapKey(workspace),
    queryFn: async (): Promise<ReviewVerdictMap> => {
      if (!skillIds.length) return {};
      const currentMap = queryClient.getQueryData<ReviewVerdictMap>(reviewVerdictMapKey(workspace)) ?? {};
      const map: ReviewVerdictMap = {};
      for (const target of reviewTargets) {
        const verdict = currentMap[target.id];
        if (verdict) map[target.id] = verdict;
      }
      const rawResults = await getRemoteReviewsBatch({ workspace, skillIds });
      await Promise.all(
        skillIds.map(async (id, i) => {
          const raw = rawResults[i];
          const target = reviewTargets[i];
          if (!raw || !target) return;
          const remote = normalizeRemoteReview(raw);
          if (!remote) return;
          // Don't clobber a local review the user just ran but hasn't synced —
          // that copy is newer than whatever is in the repo. Otherwise adopt the
          // remote copy as the cached source of truth.
          const local = await getReviewCache(workspace, target.path).catch(() => null);
          if (local && !local.synced && local.contentHash !== remote.contentHash) {
            map[id] = local.verdict;
            queryClient.setQueryData(["review-cache", workspace, target.path], local);
            return;
          }
          map[id] = remote.verdict;
          await putReviewCache(workspace, target.path, remote);
          // Seed the panel's exact query key so an open Risk tab updates live and
          // a subsequent open is zero-pending.
          queryClient.setQueryData(["review-cache", workspace, target.path], remote);
        }),
      );
      return map;
    },
    enabled: Boolean(workspace && skillIds.length && authLogin),
    staleTime: 5 * 60_000,
    refetchInterval: 5 * 60_000,
    refetchIntervalInBackground: false,
    retry: false,
  });
  const reviewVerdicts = verdictMapQuery.data ?? {};

  return (
    <>
      <WorkspacesPage
        filteredAssets={filteredAssets}
        selected={selected}
        onSelectAsset={(asset) => {
          setSelected(asset);
          setSelectedRef(undefined);
          setSelectedFile(null);
          setDetailOpen(true);
        }}
        onSelectRef={setSelectedRef}
        selectedFile={selectedFile}
        onSelectFile={setSelectedFile}
        query={query}
        setQuery={setQuery}
        workspaceMeta={workspaceMeta}
        workspaceDetail={workspaceDetail}
        workspaceRef={workspace}
        canViewFiles={Boolean(authLogin && workspaceMeta)}
        scanPending={scan.isPending}
        isRefreshing={scan.isPending && scan.data !== undefined}
        versions={workspaceDetail?.versions ?? []}
        selectedBranch={selectedRef}
        onSelectBranch={setSelectedRef}
        detailOpen={detailOpen}
        onDetailOpenChange={(open) => {
          setDetailOpen(open);
          if (!open) setSelected(null);
        }}
        reviewVerdicts={reviewVerdicts}
        detailPanel={
          selected ? (
            <Suspense
              fallback={
                <div className="flex h-full items-center justify-center text-[12px] text-[var(--fg-muted)]">
                  <Spinner size="sm" />
                </div>
              }
            >
            <SkillDetail
              asset={selected}
              detail={selectedDetail.data}
              detailPending={selectedDetail.isFetching}
              detailError={selectedDetail.error}
              selectedRef={selectedRef}
              setSelectedRef={setSelectedRef}
              selectedFile={selectedFile}
              targets={targets}
              setTargets={setTargets}
              workspaceRef={workspace}
              onSubscribeClick={() => setSubscribeOpen(true)}
              onInstall={(confirmed = false) => install.mutate(confirmed)}
              onPublish={() => publish.mutate()}
              onPublishClick={workspaceMeta ? () => setPublishOpen(true) : undefined}
              onSyncClick={workspaceMeta ? () => setSyncOpen(true) : undefined}
              onRefresh={() => {
                // Drop the persistent detail cache so the refetch hits the network.
                if (selected?.path) void clearSkillDetail(workspace, selected.path);
                queryClient.invalidateQueries({ queryKey: ["skill-detail", workspace, selected?.path, selectedRef] });
                queryClient.invalidateQueries({ queryKey: ["demo-skill-detail", selected?.path, selectedRef] });
                queryClient.invalidateQueries({ queryKey: ["skill-file-content", workspace, selectedFile, selectedRef] });
              }}
              installPending={install.isPending}
              publishPending={publish.isPending}
              installResult={install.data}
              publishResult={publish.data}
              subscriptions={subscriptions.data?.subscriptions.length ?? 0}
            />
            </Suspense>
          ) : null
        }
      />

      <SubscribeModal
        open={subscribeOpen}
        onOpenChange={setSubscribeOpen}
        asset={selected}
        workspaceFullName={workspace}
        initialTargets={targets}
        onConfirm={(input) => {
          setTargets(input.targets);
          subscribe.mutate(input);
        }}
        pending={subscribe.isPending}
      />

      <SyncSkillModal
        open={syncOpen}
        onOpenChange={setSyncOpen}
        sourceWorkspace={workspace}
        skillPath={selected?.path ?? ""}
        skillId={selected?.manifest.id ?? ""}
        sourceRef={selectedRef}
        workspaces={workspaces.data?.workspaces ?? []}
        authLogin={authLogin}
      />

      <PublishModal
        open={publishOpen}
        onOpenChange={setPublishOpen}
        asset={selected}
        workspace={workspace}
        versions={workspaceDetail?.versions ?? []}
        selectedRef={selectedRef}
        onPublish={({ bump, message }) => {
          publish.mutate();
          setPublishOpen(false);
        }}
        publishPending={publish.isPending}
      />
    </>
  );
}
