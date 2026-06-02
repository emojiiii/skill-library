import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useEffect, useState } from "react";
import { Spinner, toast } from "@heroui/react";
import {
  FEATURED_SKILLS,
  parseSource,
  type RegistrySkill,
  searchSkillsRegistry,
  SKILL_CATEGORIES,
} from "../lib/registry";
import {
  downloadSkillAsync,
  getSkillDetail,
  type SkillManifest,
} from "../lib/skill-library";
import { useLocale } from "../hooks/useLocale";
import { useAppStore } from "../state/appStore";
import { getSkillDetailFromCache, putSkillDetailInCache } from "../lib/workspaceCache";
import { formatError } from "../utils/format";
import { DiscoverPage } from "../pages/DiscoverPage";
import { InstallToToolsDialog, type InstallTargetSelection } from "../widgets/InstallToToolsDialog";

// Lazy-loaded so file preview/tree code stays off the startup path.
const DiscoverSkillDetail = lazy(() => import("../widgets/DiscoverSkillDetail").then((m) => ({ default: m.DiscoverSkillDetail })));

/** Debounce a changing value by `delay` ms. */
function useDebounced<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = window.setTimeout(() => setDebounced(value), delay);
    return () => window.clearTimeout(id);
  }, [value, delay]);
  return debounced;
}

export function DiscoverRoute() {
  const { t } = useLocale();
  const queryClient = useQueryClient();
  const targets = useAppStore((s) => s.targets);
  const setTargets = useAppStore((s) => s.setTargets);

  const [query, setQuery] = useState("");
  const [activeCategory, setActiveCategory] = useState<string | null>(null);
  const [selected, setSelected] = useState<RegistrySkill | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);
  const [installOpen, setInstallOpen] = useState(false);

  const debouncedQuery = useDebounced(query, 350);

  // Card body click → open the detail modal.
  const handleSelect = (skill: RegistrySkill) => {
    setSelected(skill);
    setDetailOpen(true);
  };

  // Hover "install" button → install directly, skipping the modal. We still
  // set `selected` so the detail query loads the manifest the install needs;
  // the InstallToToolsDialog stays hidden until the manifest is ready.
  const handleInstallClick = (skill: RegistrySkill) => {
    setSelected(skill);
    setInstallOpen(true);
  };

  // Manual typing in the search box clears any active category selection.
  const handleSetQuery = (value: string) => {
    setActiveCategory(null);
    setQuery(value);
  };

  // Clicking a category chip runs its canned query through the search path.
  // Clicking "All" (null) clears both the category and the query → featured.
  const handleSelectCategory = (categoryId: string | null) => {
    setActiveCategory(categoryId);
    const cat = categoryId ? SKILL_CATEGORIES.find((c) => c.id === categoryId) : null;
    setQuery(cat?.query ?? "");
  };

  // Registry search. Falls back to the curated featured list when the query is
  // too short or the registry is unreachable, so the screen is never blank.
  const search = useQuery({
    queryKey: ["registry-search", debouncedQuery],
    queryFn: () => searchSkillsRegistry(debouncedQuery),
    enabled: debouncedQuery.trim().length >= 2,
    staleTime: 5 * 60 * 1000,
  });

  const isFeatured = debouncedQuery.trim().length < 2;
  const skills = isFeatured ? FEATURED_SKILLS : search.data ?? [];

  // Anonymous skill detail for the selected card → drives the safety card.
  const source = selected ? parseSource(selected.source) : null;
  const detail = useQuery({
    queryKey: ["discover-skill-detail", selected?.source, selected?.skillId],
    queryFn: async () => {
      // Render instantly from cache if we've fetched this skill before.
      const cached = await getSkillDetailFromCache<Awaited<ReturnType<typeof getSkillDetail>>>(
        selected!.source,
        selected!.skillId,
      );
      if (cached) {
        // Refresh in the background so a stale entry self-heals.
        void getSkillDetail({ workspace: selected!.source, skillPath: selected!.skillId })
          .then((fresh) => putSkillDetailInCache(selected!.source, selected!.skillId, undefined, fresh))
          .catch(() => undefined);
        return cached;
      }
      const fresh = await getSkillDetail({
        workspace: selected!.source,
        // Best-effort skill path: registries key by skillId; the backend
        // resolves the directory by scanning + matching id/basename.
        skillPath: selected!.skillId,
      });
      await putSkillDetailInCache(selected!.source, selected!.skillId, undefined, fresh);
      return fresh;
    },
    enabled: Boolean(source && selected),
    staleTime: 5 * 60 * 1000,
    // A 404 here means the skill genuinely isn't there — don't retry.
    retry: 0,
  });

  const manifest: SkillManifest | null = detail.data?.asset.manifest ?? null;

  // Install = start an async download + install. Returns immediately; progress
  // shows up in My Skills (downloading bar → installed / retry on error).
  // Empty targets = download locally, deploy to no tools.
  const install = useMutation({
    mutationFn: async (selection: InstallTargetSelection) => {
      if (!selected) throw new Error("no skill selected");
      // Prefer the backend-resolved in-repo path; fall back to the registry id.
      const skillPath = detail.data?.asset.path ?? selected.skillId;
      await downloadSkillAsync({
        workspace: selected.source,
        assetId: manifest?.id ?? selected.skillId,
        skillPath,
        version: manifest?.version,
        name: manifest?.name ?? selected.name,
        description: manifest?.description,
        targets: selection.targets,
        projectTargets: selection.projectTargets,
      });
    },
    onSuccess: () => {
      setInstallOpen(false);
      toast.info(t("discover.downloadStarted"));
      queryClient.invalidateQueries({ queryKey: ["db-skills"] });
    },
    onError: (err) => {
      const code = (err as { code?: string })?.code;
      if (code === "already_downloading") {
        toast.warning(t("discover.alreadyDownloading"));
      } else if (code === "already_installed") {
        toast.warning(t("discover.alreadyInstalled"));
      } else {
        toast.danger(formatError(err));
      }
    },
  });

  return (
    <>
      <DiscoverPage
        query={query}
        setQuery={handleSetQuery}
        skills={skills}
        loading={search.isFetching}
        error={search.error ? formatError(search.error) : null}
        isFeatured={isFeatured}
        activeCategory={activeCategory}
        onSelectCategory={handleSelectCategory}
        onSelect={handleSelect}
        onInstall={handleInstallClick}
        detailPanel={
          selected ? (
            <Suspense
              fallback={
                <div className="flex h-full items-center justify-center text-[12px] text-[var(--fg-muted)]">
                  <Spinner size="sm" />
                </div>
              }
            >
              <DiscoverSkillDetail
                selected={selected}
                detail={detail.data}
                loading={detail.isLoading && !manifest}
                error={detail.error}
                installPending={install.isPending}
                onInstallClick={() => setInstallOpen(true)}
              />
            </Suspense>
          ) : null
        }
        detailOpen={detailOpen}
        onDetailOpenChange={(open) => {
          setDetailOpen(open);
          if (!open && !installOpen) setSelected(null);
        }}
      />

      <InstallToToolsDialog
        open={installOpen}
        onOpenChange={(value) => {
          setInstallOpen(value);
          if (!value) {
            // If the modal isn't showing this skill, drop the selection so a
            // stale detail query doesn't linger.
            if (!detailOpen) setSelected(null);
          }
        }}
        manifest={manifest}
        loading={detail.isLoading}
        sourceLabel={selected?.source ?? ""}
        fallbackName={selected?.name}
        defaultTargets={targets}
        pending={install.isPending}
        onConfirm={(selection) => {
          if (!selection.projectTargets.length) setTargets(selection.targets);
          install.mutate(selection);
        }}
      />
    </>
  );
}
