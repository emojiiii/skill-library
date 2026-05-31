import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { Download } from "lucide-react";
import { Button, toast } from "@heroui/react";
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
} from "../lib/teamai";
import { useLocale } from "../hooks/useLocale";
import { useAppStore } from "../state/appStore";
import { getSkillDetailFromCache, putSkillDetailInCache } from "../lib/workspaceCache";
import { formatError } from "../utils/format";
import { DiscoverPage } from "../pages/DiscoverPage";
import { SkillSafetyCard } from "../widgets/SkillSafetyCard";
import { InstallToToolsDialog } from "../widgets/InstallToToolsDialog";

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
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [installOpen, setInstallOpen] = useState(false);

  const debouncedQuery = useDebounced(query, 350);

  // Card body click → open the detail drawer.
  const handleSelect = (skill: RegistrySkill) => {
    setSelected(skill);
    setDrawerOpen(true);
  };

  // Hover "install" button → install directly, skipping the drawer. We still
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
    mutationFn: async (chosenTargets: string[]) => {
      if (!selected || !manifest) throw new Error("no skill selected");
      // Prefer the backend-resolved in-repo path; fall back to the registry id.
      const skillPath = detail.data?.asset.path ?? selected.skillId;
      await downloadSkillAsync({
        workspace: selected.source,
        assetId: manifest.id,
        skillPath,
        version: manifest.version,
        name: manifest.name,
        description: manifest.description,
        targets: chosenTargets,
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

  const detailPanel = useMemo(() => {
    if (!selected) return null;
    return (
      <div className="flex h-full flex-col">
        <div className="border-b border-[var(--line)] px-5 py-4">
          <div className="text-[15px] font-semibold tracking-tight text-[var(--fg)]">
            {selected.name}
          </div>
          <div className="mt-0.5 text-[11.5px] text-[var(--fg-muted)]">{selected.source}</div>
        </div>
        <div className="scroll-area flex-1 px-5 py-4">
          {detail.isLoading ? (
            <div className="py-8 text-center text-[12px] text-[var(--fg-muted)]">
              {t("common.loading")}
            </div>
          ) : manifest ? (
            <div className="space-y-4">
              {manifest.description ? (
                <p className="text-[13px] leading-[1.6] text-[var(--fg-secondary)]">
                  {manifest.description}
                </p>
              ) : null}
              <SkillSafetyCard manifest={manifest} />
            </div>
          ) : (
            <div className="rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-[12px] text-[var(--warning)]">
              {detail.error ? formatError(detail.error) : t("discover.detailUnavailable")}
            </div>
          )}
        </div>
        <div className="border-t border-[var(--line)] px-5 py-3">
          <Button
            fullWidth
            className="h-10"
            isDisabled={!manifest}
            onPress={() => {
              setInstallOpen(true);
            }}
          >
            <Download size={15} />
            {t("discover.install")}
          </Button>
        </div>
      </div>
    );
  }, [selected, detail.isLoading, detail.error, manifest, t]);

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
        selectedId={selected?.id ?? null}
        onSelect={handleSelect}
        onInstall={handleInstallClick}
        detailPanel={detailPanel}
        detailOpen={drawerOpen}
        onDetailOpenChange={(open) => {
          setDrawerOpen(open);
          if (!open && !installOpen) setSelected(null);
        }}
      />

      <InstallToToolsDialog
        open={installOpen}
        onOpenChange={(value) => {
          setInstallOpen(value);
          if (!value) {
            // If the drawer isn't showing this skill, drop the selection so a
            // stale detail query doesn't linger.
            if (!drawerOpen) setSelected(null);
          }
        }}
        manifest={manifest}
        loading={detail.isLoading}
        sourceLabel={selected?.source ?? ""}
        defaultTargets={targets}
        pending={install.isPending}
        onConfirm={(chosenTargets) => {
          setTargets(chosenTargets);
          install.mutate(chosenTargets);
        }}
      />
    </>
  );
}
