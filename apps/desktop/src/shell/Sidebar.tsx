import { Link, useRouterState } from "@tanstack/react-router";
import {
  Activity,
  Bell,
  ChevronRight,
  GitPullRequestArrow,
  HardDrive,
  PackageOpen,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  Terminal,
  UsersRound,
} from "lucide-react";
import { type ReactNode, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import type { StoredWorkspace } from "../lib/teamai";
import { type AppPage, buildNavPath, navRoutes, routeToPage, workspaceFromPathname } from "../utils/navigation";
import { WorkspacePicker } from "./WorkspacePicker";

const navIcon: Record<AppPage, ReactNode> = {
  workspaces: <PackageOpen size={15} />,
  subscriptions: <Bell size={15} />,
  installed: <HardDrive size={15} />,
  publish: <GitPullRequestArrow size={15} />,
  invitations: <UsersRound size={15} />,
  activity: <Activity size={15} />,
  cli: <Terminal size={15} />,
};

type NavGroup = { titleKey: string; pages: AppPage[] };

const navGroups: NavGroup[] = [
  { titleKey: "nav.workspace", pages: ["workspaces", "publish", "invitations", "activity"] },
  { titleKey: "nav.personal", pages: ["installed", "subscriptions"] },
  { titleKey: "nav.tools", pages: ["cli"] },
];

const navLabelKeys: Record<AppPage, string> = {
  workspaces: "nav.skills",
  publish: "nav.publishPrs",
  invitations: "nav.members",
  activity: "nav.activity",
  subscriptions: "nav.subscriptions",
  installed: "nav.local",
  cli: "nav.cli",
};

export function Sidebar({
  current,
  saved,
  onSelectWorkspace,
  onOpenAddDialog,
  counts,
  authLogin,
  onOpenAccount,
}: {
  current: { full_name: string; visibility?: string; permission?: string } | null;
  saved: StoredWorkspace[];
  onSelectWorkspace: (workspace: { full_name: string }) => void;
  onOpenAddDialog: () => void;
  counts: Partial<Record<AppPage, number>>;
  authLogin: string | null | undefined;
  onOpenAccount: () => void;
}) {
  const { t } = useLocale();
  const [collapsed, setCollapsed] = useState(false);
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const currentPage = routeToPage(pathname);
  const currentWorkspace = workspaceFromPathname(pathname);

  return (
    <aside className={`app-shell__sidebar ${collapsed ? "is-collapsed" : ""}`}>
      {!collapsed ? (
        <WorkspacePicker
          current={current}
          saved={saved}
          onSelect={onSelectWorkspace}
          onOpenAddDialog={onOpenAddDialog}
        />
      ) : (
        <div className="sidebar-collapsed-avatar">
          <span className="grid size-8 place-items-center rounded-lg bg-[var(--brand-soft)] text-[10px] font-bold text-[var(--brand-fg)]">
            {current?.full_name?.slice(0, 2).toUpperCase() ?? "—"}
          </span>
        </div>
      )}

      <div className="sidebar-scroll">
        {navGroups.map((group) => (
          <div key={group.titleKey} className="mb-2">
            {!collapsed ? <div className="sidebar-section-title">{t(group.titleKey)}</div> : null}
            {group.pages.map((page) => {
              const route = navRoutes.find((entry) => entry.page === page);
              if (!route) return null;
              const count = counts[page];
              const href = buildNavPath(route, currentWorkspace ?? current?.full_name ?? null);
              const label = t(navLabelKeys[page]);
              return (
                <NavLink
                  key={page}
                  to={href}
                  label={label}
                  icon={navIcon[page]}
                  active={currentPage === page}
                  count={typeof count === "number" ? count : undefined}
                  collapsed={collapsed}
                />
              );
            })}
          </div>
        ))}
      </div>

      <div className="sidebar-footer">
        <button
          type="button"
          onClick={() => setCollapsed((v) => !v)}
          className="sidebar-collapse-btn"
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {collapsed ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
          {!collapsed ? <span>{t("nav.collapse")}</span> : null}
        </button>

        <button
          type="button"
          onClick={onOpenAccount}
          className={`sidebar-user-btn ${collapsed ? "is-collapsed" : ""}`}
        >
          <span className="grid size-7 shrink-0 place-items-center rounded-full bg-[var(--brand-soft)] text-[10px] font-semibold text-[var(--brand-fg)]">
            {(authLogin ?? "?").slice(0, 2).toUpperCase()}
          </span>
          {!collapsed ? (
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[12.5px] font-medium text-[var(--fg)]">
                {authLogin ? `@${authLogin}` : t("sidebar.notSignedIn")}
              </span>
              <span className="block truncate text-[11px] text-[var(--fg-muted)]">
                {authLogin ? t("sidebar.githubConnected") : t("sidebar.clickToSignIn")}
              </span>
            </span>
          ) : null}
          {!collapsed ? <Settings size={14} className="text-[var(--fg-muted)]" /> : null}
        </button>
      </div>
    </aside>
  );
}

function NavLink({
  to,
  label,
  icon,
  active,
  count,
  collapsed,
}: {
  to: string;
  label: string;
  icon: ReactNode;
  active?: boolean;
  count?: number;
  collapsed?: boolean;
}) {
  return (
    <Link to={to} activeOptions={{ exact: true }} activeProps={{}} inactiveProps={{}} className={`sidebar-link ${active ? "active" : ""} ${collapsed ? "is-collapsed" : ""}`} title={collapsed ? label : undefined}>
      <span className="sidebar-link__icon">{icon}</span>
      {!collapsed ? <span className="flex-1 truncate">{label}</span> : null}
      {!collapsed && typeof count === "number" && count > 0 ? <span className="sidebar-link__count">{count}</span> : null}
      {!collapsed && active ? <ChevronRight size={12} className="opacity-60" /> : null}
    </Link>
  );
}
