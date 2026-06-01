import { BadgeCheck, Download, Eye } from "lucide-react";
import type { KeyboardEvent } from "react";
import type { RegistrySkill } from "../lib/registry";
import { useLocale } from "../hooks/useLocale";
import { formatInstalls } from "../utils/format";

/**
 * App-store style card for a skill in the discover grid. Hovering (or focusing)
 * reveals two centered actions so the affordance is explicit:
 *   - View  → opens the detail drawer
 *   - Install → installs directly, skipping the drawer
 * Clicking the card body (outside the buttons) also opens the detail drawer.
 *
 * The outer element is a div (not a button) because it contains inner buttons —
 * nested buttons are invalid HTML. Keyboard access is wired via
 * role/tabIndex/onKeyDown.
 */
export function ConsumerSkillCard({
  skill,
  onSelect,
  onInstall,
}: {
  skill: RegistrySkill;
  onSelect: () => void;
  onInstall: () => void;
}) {
  const { t } = useLocale();

  const handleKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onSelect();
    }
  };

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={handleKeyDown}
      className="group relative flex w-full cursor-pointer flex-col gap-2 rounded-[12px] border border-[var(--line)] bg-[var(--bg-elevated)] p-4 text-left transition-colors hover:border-[var(--brand)]/50 hover:bg-[var(--bg-soft)] focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-soft)]"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex min-w-0 items-center gap-1.5">
          <span className="truncate text-[14px] font-semibold tracking-tight text-[var(--fg)]">
            {skill.name}
          </span>
          {skill.isOfficial ? (
            <BadgeCheck size={14} className="shrink-0 text-[var(--brand)]" />
          ) : null}
        </div>
      </div>
      <div className="truncate text-[11.5px] text-[var(--fg-muted)]">{skill.source}</div>
      <div className="mt-auto flex items-center justify-between pt-1">
        <span className="flex items-center gap-1 text-[11.5px] text-[var(--fg-muted)]">
          <Download size={12} />
          {formatInstalls(skill.installs)} {t("discover.installs")}
        </span>
      </div>

      {/* Hover overlay: View (opens drawer) + Install (direct) */}
      <div className="pointer-events-none absolute inset-0 grid place-items-center gap-2 rounded-[12px] bg-[var(--bg-elevated)]/60 opacity-0 backdrop-blur-[1px] transition-opacity duration-150 group-hover:opacity-100 group-focus-visible:opacity-100 focus-within:opacity-100">
        <div className="flex items-center gap-2">
          <button
            type="button"
            aria-label={t("discover.view")}
            title={t("discover.view")}
            onClick={(e) => {
              e.stopPropagation();
              onSelect();
            }}
            className="pointer-events-auto inline-flex items-center gap-1.5 rounded-full border border-[var(--line)] bg-[var(--bg-elevated)] px-3.5 py-2 text-[12.5px] font-medium text-[var(--fg)] shadow-sm transition-transform hover:scale-[1.03] active:scale-95"
          >
            <Eye size={14} />
            {t("discover.view")}
          </button>
          <button
            type="button"
            aria-label={t("discover.install")}
            title={t("discover.install")}
            onClick={(e) => {
              e.stopPropagation();
              onInstall();
            }}
            className="pointer-events-auto inline-flex items-center gap-1.5 rounded-full bg-[var(--brand)] px-3.5 py-2 text-[12.5px] font-medium text-[var(--accent-foreground)] shadow-md transition-transform hover:scale-[1.03] active:scale-95"
          >
            <Download size={14} />
            {t("discover.installShort")}
          </button>
        </div>
      </div>
    </div>
  );
}
