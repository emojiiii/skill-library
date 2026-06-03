import { Check, ChevronsUpDown, Plus, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { StoredWorkspace } from "../lib/skill-library";
import { useLocale } from "../hooks/useLocale";
import {
  workspaceKey,
  workspaceProviderShortLabel,
} from "../lib/providers";
import { workspaceColor, workspaceInitials } from "../utils/workspace-visual";

export function WorkspacePicker({
  current,
  saved,
  onSelect,
  onOpenAddDialog,
}: {
  current: { provider?: string; full_name: string; visibility?: string; permission?: string } | null;
  saved: StoredWorkspace[];
  onSelect: (workspace: StoredWorkspace) => void;
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
  const currentKey = current?.provider && current.full_name
    ? workspaceKey({ provider: current.provider, full_name: current.full_name })
    : current?.full_name;
  const owner = current?.full_name.split("/")[0] ?? "";
  const repo = current?.full_name.split("/")[1] ?? "";
  const triggerColor = hasCurrent
    ? workspaceColor(current!.full_name)
    : { bg: "var(--bg-active)", fg: "var(--fg-muted)" };
  const triggerInitials = hasCurrent ? workspaceInitials({ owner, repo }) : "-";

  return (
    <div ref={containerRef} className="workspace-picker-container">
      {/* Trigger — flat, no border, like Codex sidebar header */}
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="workspace-picker-trigger"
      >
        <span
          className="workspace-picker-trigger__avatar"
          style={{ background: triggerColor.bg, color: triggerColor.fg }}
        >
          {triggerInitials}
        </span>
        <span className="workspace-picker-trigger__text">
          <span className="workspace-picker-trigger__name-row">
            <span className="workspace-picker-trigger__name">
              {hasCurrent ? current!.full_name : t("picker.noWorkspaces")}
            </span>
            {hasCurrent ? (
              <span className="workspace-provider-badge is-small">
                {workspaceProviderShortLabel(current?.provider)}
              </span>
            ) : null}
          </span>
          <span className="workspace-picker-trigger__sub">
            {hasCurrent
              ? [current?.permission, current?.visibility].filter(Boolean).join(" · ") || "-"
              : t("picker.addWorkspace")}
          </span>
        </span>
        <ChevronsUpDown size={14} className="workspace-picker-trigger__chevron" />
      </button>

      {/* Dropdown */}
      {open && (
        <>
          <div className="workspace-popover__overlay" onClick={() => setOpen(false)} />
          <div className="workspace-popover">
            {/* Search */}
            {saved.length > 3 && (
              <div className="workspace-popover__search">
                <Search size={12} className="shrink-0 text-[var(--fg-muted)]" />
                <input
                  autoFocus
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder={t("picker.searchPlaceholder")}
                  className="workspace-popover__search-input"
                />
              </div>
            )}

            {/* List */}
            <div className="workspace-popover__list">
              {filtered.length ? (
                filtered.map((ws) => {
                  const active = workspaceKey(ws) === currentKey || ws.full_name === currentKey;
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
                      className={`workspace-popover__item ${active ? "is-active" : ""}`}
                    >
                      <span
                        className="workspace-popover__avatar"
                        style={{ background: color.bg, color: color.fg }}
                      >
                        {initials}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="workspace-popover__item-name-row">
                          <span className="workspace-popover__item-name">{ws.full_name}</span>
                          <span className="workspace-provider-badge is-small">
                            {workspaceProviderShortLabel(ws.provider)}
                          </span>
                        </span>
                        <span className="workspace-popover__item-sub">
                          {ws.permission} &middot; {ws.visibility}
                        </span>
                      </span>
                      {active && <Check size={13} className="shrink-0 text-[var(--brand)]" />}
                    </button>
                  );
                })
              ) : (
                <div className="workspace-popover__empty">
                  {saved.length ? t("picker.noMatches") : t("picker.noWorkspaces")}
                </div>
              )}
            </div>

            {/* Footer: add workspace */}
            <div className="workspace-popover__footer">
              <button
                type="button"
                onClick={() => {
                  setOpen(false);
                  onOpenAddDialog();
                }}
                className="workspace-popover__footer-button"
              >
                <Plus size={13} />
                {t("picker.addFromGithub")}
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
