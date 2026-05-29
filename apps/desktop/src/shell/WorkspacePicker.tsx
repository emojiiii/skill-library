import { Check, ChevronsUpDown, Plus, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { StoredWorkspace } from "../lib/teamai";
import { useLocale } from "../hooks/useLocale";
import { workspaceColor, workspaceInitials } from "../utils/workspace-visual";

export function WorkspacePicker({
  current,
  saved,
  onSelect,
  onOpenAddDialog,
}: {
  current: { full_name: string; visibility?: string; permission?: string } | null;
  saved: StoredWorkspace[];
  onSelect: (workspace: { full_name: string }) => void;
  onOpenAddDialog: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);
  const { t } = useLocale();

  // Close on outside click / Escape
  useEffect(() => {
    if (!open) return;
    const onDocClick = (event: MouseEvent) => {
      if (!containerRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDocClick);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDocClick);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  useEffect(() => {
    if (!open) setQuery("");
  }, [open]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return saved;
    return saved.filter((ws) => ws.full_name.toLowerCase().includes(needle));
  }, [saved, query]);

  const hasCurrent = Boolean(current?.full_name);
  const owner = current?.full_name.split("/")[0] ?? "";
  const repo = current?.full_name.split("/")[1] ?? "";
  const triggerColor = hasCurrent
    ? workspaceColor(current!.full_name)
    : { bg: "var(--bg-active)", fg: "var(--fg-muted)" };
  const triggerInitials = hasCurrent ? workspaceInitials({ owner, repo }) : "-";

  return (
    <div ref={containerRef} className="relative">
      {/* Trigger button */}
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-3 rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2.5 text-left transition-colors hover:bg-[var(--bg-soft)]"
      >
        <span
          className="grid size-8 shrink-0 place-items-center rounded-md text-[11px] font-semibold"
          style={{ background: triggerColor.bg, color: triggerColor.fg }}
        >
          {triggerInitials}
        </span>
        <span className="min-w-0 flex-1">
          <div className="truncate text-[13px] font-semibold">
            {hasCurrent ? current!.full_name : t("picker.noWorkspaces")}
          </div>
          <div className="truncate text-[11px] text-[var(--fg-muted)]">
            {hasCurrent
              ? [current?.permission, current?.visibility].filter(Boolean).join(" · ") || "-"
              : t("picker.addWorkspace")}
          </div>
        </span>
        <ChevronsUpDown size={14} className="shrink-0 text-[var(--fg-muted)]" />
      </button>

      {/* Dropdown */}
      {open && (
        <div className="absolute left-0 right-0 top-full z-50 mt-1 rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] shadow-lg">
          {/* Search */}
          {saved.length > 3 && (
            <div className="flex items-center gap-2 border-b border-[var(--line)] px-3 py-2">
              <Search size={12} className="shrink-0 text-[var(--fg-muted)]" />
              <input
                autoFocus
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Find workspace..."
                className="min-w-0 flex-1 bg-transparent text-[12px] outline-none placeholder:text-[var(--fg-muted)]"
              />
            </div>
          )}

          {/* List */}
          <div className="max-h-[240px] overflow-y-auto">
            {filtered.length ? (
              filtered.map((ws) => {
                const active = ws.full_name === current?.full_name;
                const color = workspaceColor(ws.full_name);
                const initials = workspaceInitials({
                  owner: ws.full_name.split("/")[0] ?? "",
                  repo: ws.full_name.split("/")[1] ?? "",
                });
                return (
                  <button
                    key={ws.full_name}
                    type="button"
                    onClick={() => {
                      onSelect(ws);
                      setOpen(false);
                    }}
                    className={`flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-[var(--bg-soft)] ${
                      active ? "bg-[var(--brand-soft)]" : ""
                    }`}
                  >
                    <span
                      className="grid size-7 shrink-0 place-items-center rounded-md text-[10px] font-semibold"
                      style={{ background: color.bg, color: color.fg }}
                    >
                      {initials}
                    </span>
                    <span className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium">{ws.full_name}</div>
                      <div className="text-[11px] text-[var(--fg-muted)]">
                        {ws.permission} &middot; {ws.visibility}
                      </div>
                    </span>
                    {active && <Check size={13} className="shrink-0 text-[var(--brand)]" />}
                  </button>
                );
              })
            ) : (
              <div className="px-3 py-4 text-center text-[12px] text-[var(--fg-muted)]">
                {saved.length ? t("picker.noMatches") : t("picker.noWorkspaces")}
              </div>
            )}
          </div>

          {/* Footer: add workspace */}
          <div className="border-t border-[var(--line)]">
            <button
              type="button"
              onClick={() => {
                setOpen(false);
                onOpenAddDialog();
              }}
              className="flex w-full items-center gap-2 px-3 py-2.5 text-[12px] text-[var(--fg-muted)] transition-colors hover:bg-[var(--bg-soft)] hover:text-[var(--fg)]"
            >
              <Plus size={13} />
              {t("picker.addFromGithub")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
