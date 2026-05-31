import { Search } from "lucide-react";
import { Chip, Drawer } from "@heroui/react";
import type { ReactNode } from "react";
import { SKILL_CATEGORIES, type RegistrySkill } from "../lib/registry";
import { useLocale } from "../hooks/useLocale";
import { ConsumerSkillCard } from "../widgets/ConsumerSkillCard";

/**
 * Consumer home: an app-store style grid of skills. Search box up top, a row of
 * category chips below it, cards below that, and a detail drawer (passed in by
 * the route) that slides in from the right when a skill is selected. No source
 * code, no Git, no workspaces.
 */
export function DiscoverPage({
  query,
  setQuery,
  skills,
  loading,
  error,
  isFeatured,
  activeCategory,
  onSelectCategory,
  selectedId,
  onSelect,
  onInstall,
  detailPanel,
  detailOpen,
  onDetailOpenChange,
}: {
  query: string;
  setQuery: (value: string) => void;
  skills: RegistrySkill[];
  loading: boolean;
  error: string | null;
  isFeatured: boolean;
  activeCategory: string | null;
  onSelectCategory: (categoryId: string | null) => void;
  selectedId: string | null;
  onSelect: (skill: RegistrySkill) => void;
  onInstall: (skill: RegistrySkill) => void;
  detailPanel: ReactNode;
  detailOpen: boolean;
  onDetailOpenChange: (open: boolean) => void;
}) {
  const { t } = useLocale();

  const heading = activeCategory
    ? t(SKILL_CATEGORIES.find((c) => c.id === activeCategory)?.labelKey ?? "discover.results")
    : isFeatured
      ? t("discover.featured")
      : t("discover.results");

  return (
    <div className="flex h-full min-h-0">
      {/* Left: search + grid */}
      <section className="flex min-w-0 flex-1 flex-col">
        <div className="border-b border-[var(--line)] bg-[var(--bg-elevated)] px-6 py-4">
          <div className="relative mx-auto max-w-2xl">
            <Search size={15} className="absolute left-3.5 top-1/2 -translate-y-1/2 text-[var(--fg-muted)]" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("discover.searchPlaceholder")}
              className="w-full rounded-lg border border-[var(--line)] bg-[var(--bg)] py-2.5 pl-10 pr-3 text-[14px] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
            />
          </div>

          {/* Category chips */}
          <div className="mx-auto mt-3 flex max-w-2xl flex-wrap gap-1.5">
            <button type="button" onClick={() => onSelectCategory(null)}>
              <Chip
                size="sm"
                color={activeCategory === null && !query ? "accent" : "default"}
                variant={activeCategory === null && !query ? "primary" : "soft"}
              >
                {t("discover.cat.all")}
              </Chip>
            </button>
            {SKILL_CATEGORIES.map((cat) => (
              <button type="button" key={cat.id} onClick={() => onSelectCategory(cat.id)}>
                <Chip
                  size="sm"
                  color={activeCategory === cat.id ? "accent" : "default"}
                  variant={activeCategory === cat.id ? "primary" : "soft"}
                >
                  {t(cat.labelKey)}
                </Chip>
              </button>
            ))}
          </div>
        </div>

        <div className="scroll-area flex-1 px-6 py-5">
          <div className="mx-auto max-w-5xl">
            <div className="mb-3 flex items-center justify-between">
              <h2 className="text-[12px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                {heading}
              </h2>
              {loading ? (
                <span className="text-[11.5px] text-[var(--brand)]">{t("discover.searching")}</span>
              ) : null}
            </div>

            {error ? (
              <div className="mb-3 rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-[12px] text-[var(--warning)]">
                {error}
              </div>
            ) : null}

            {skills.length ? (
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
                {skills.map((skill) => (
                  <ConsumerSkillCard
                    key={skill.id}
                    skill={skill}
                    selected={selectedId === skill.id}
                    onSelect={() => onSelect(skill)}
                    onInstall={() => onInstall(skill)}
                  />
                ))}
              </div>
            ) : !loading ? (
              <div className="empty-state rounded-md border border-dashed border-[var(--line)]">
                <div className="empty-state__title">{t("discover.empty")}</div>
                <div>{t("discover.empty.desc")}</div>
              </div>
            ) : null}
          </div>
        </div>
      </section>

      {/* Right: detail drawer (slides in when a skill is selected) */}
      <Drawer isOpen={detailOpen} onOpenChange={onDetailOpenChange}>
        <Drawer.Backdrop>
          <Drawer.Content placement="right">
            <Drawer.Dialog className="flex h-full w-[min(440px,92vw)] flex-col bg-[var(--bg-elevated)] outline-none">
              {detailPanel}
            </Drawer.Dialog>
          </Drawer.Content>
        </Drawer.Backdrop>
      </Drawer>
    </div>
  );
}
