import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";
import { useSyncPoller } from "../hooks/useSyncPoller";
import { useWindowFocus } from "../hooks/useWindowFocus";
import { useLocalStorage } from "../hooks/useLocalStorage";
import { getSkillsListCache, setSkillsListCache } from "../lib/workspaceCache";
import {
  getSkillDetail,
  getWorkspaceDetail,
  installSkill,
  previewPublish,
  scanGithubWorkspace,
  scanWorkspace,
  type SkillAsset,
  subscribeWorkspaceSkill,
  type Workspace,
  type WorkspaceDetail,
  listWorkspaces,
} from "../lib/teamai";
import { WorkspacesPage } from "../pages/WorkspacesPage";
import { PublishModal } from "../widgets/PublishModal";
import { SkillDetail } from "../widgets/SkillDetail";
import { SubscribeModal, type Channel, type UpdatePolicy } from "../widgets/SubscribeModal";
import { SyncSkillModal } from "../widgets/SyncSkillModal";
import { useWorkspace } from "../context/WorkspaceContext";
import { useAppStore } from "../state/appStore";

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
  const [subscribeOpen, setSubscribeOpen] = useState(false);
  const [syncOpen, setSyncOpen] = useState(false);
  const [publishOpen, setPublishOpen] = useState(false);
  const [demoWorkspaceDetail, setDemoWorkspaceDetail] = useState<WorkspaceDetail | null>(null);
  const [cachedSkills, setCachedSkills] = useState<{ workspace: string; skills: SkillAsset[] }>({ workspace: "", skills: [] });
  const prevAssetsRef = useRef<SkillAsset[]>([]);
  const initialLoadDone = useRef(false);

  // Simple setters that also persist
  const setSelected = (asset: SkillAsset | null) => {
    setSelectedRaw(asset);
    if (asset) setPersistedSkillId(asset.manifest.id);
  };

  const setSelectedFile = (file: string | null) => {
    setSelectedFileRaw(file);
    setPersistedFile(file);
  };

  // --- Scan mutation ---
  const scan = useMutation({
    mutationFn: async (request: ScanRequest) => {
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
        const fallback = await scanGithubWorkspace({ workspace: request.value });
        return { workspace: fallback.workspace, skills: fallback.skills, detail: null, fromCache: false };
      }
    },
    onSuccess: (result) => {
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
    queryFn: () => getSkillDetail({ workspace, skillPath: selected?.path ?? "", refName: selectedRef }),
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
  const assets = currentAssets.length > 0 ? currentAssets : (prevAssetsRef.current.length > 0 ? prevAssetsRef.current : (cachedSkills.workspace === workspace ? cachedSkills.skills : []));

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

  return (
    <>
      <WorkspacesPage
        filteredAssets={filteredAssets}
        selected={selected}
        onSelectAsset={setSelected}
        onSelectRef={setSelectedRef}
        selectedFile={selectedFile}
        onSelectFile={setSelectedFile}
        query={query}
        setQuery={setQuery}
        workspaceMeta={workspaceMeta}
        workspaceDetail={workspaceDetail}
        workspaceRef={workspace}
        scanPending={scan.isPending}
        isRefreshing={scan.isPending && scan.data !== undefined}
        versions={workspaceDetail?.versions ?? []}
        selectedBranch={selectedRef}
        onSelectBranch={setSelectedRef}
        detailPanel={
          selected ? (
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
