import { Button, Input, Modal } from "@heroui/react";
import { Plus, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { Workspace } from "../lib/teamai";
import { workspaceColor, workspaceInitials } from "../utils/workspace-visual";
import { Pill } from "../widgets/Pill";

export function AddWorkspaceDialog({
  open,
  onOpenChange,
  remote,
  remoteFetching,
  remoteEnabled,
  query,
  setQuery,
  onAddRemote,
  isAddingFullName,
  manualPath,
  setManualPath,
  onAddManual,
  manualPending,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  remote: Workspace[];
  remoteFetching: boolean;
  remoteEnabled: boolean;
  query: string;
  setQuery: (value: string) => void;
  onAddRemote: (workspace: Workspace) => void;
  isAddingFullName?: string;
  manualPath: string;
  setManualPath: (value: string) => void;
  onAddManual: () => void;
  manualPending: boolean;
}) {
  const [tab, setTab] = useState<"github" | "manual">("github");

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return remote;
    return remote.filter((ws) => ws.full_name.toLowerCase().includes(needle));
  }, [remote, query]);

  useEffect(() => {
    if (open) {
      setTab(remoteEnabled ? "github" : "manual");
    }
  }, [open, remoteEnabled]);

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="md">
          <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold tracking-tight">Add a workspace</Modal.Heading>
              <div className="mt-1 text-[12px] text-[var(--fg-muted)]">
                A workspace is just a Git repository. Pick one you have access to.
              </div>
            </Modal.Header>

            <div className="border-b border-[var(--line)] px-5">
              <div className="flex gap-4">
                <TabButton active={tab === "github"} onClick={() => setTab("github")}>
                  From GitHub
                </TabButton>
                <TabButton active={tab === "manual"} onClick={() => setTab("manual")}>
                  Manual / Local
                </TabButton>
              </div>
            </div>

            <Modal.Body className="px-5 py-4">
              {tab === "github" ? (
                remoteEnabled ? (
                  <div className="space-y-3">
                    <div className="relative">
                      <Search size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--fg-muted)]" />
                      <input
                        autoFocus
                        value={query}
                        onChange={(event) => setQuery(event.target.value)}
                        placeholder="Search your GitHub repos…"
                        className="w-full rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] py-2 pl-8 pr-3 text-[13px] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                      />
                    </div>
                    <div className="max-h-[320px] overflow-y-auto rounded-md border border-[var(--line)]">
                      {remoteFetching && !remote.length ? (
                        <div className="px-3 py-6 text-center text-[12px] text-[var(--fg-muted)]">Loading…</div>
                      ) : filtered.length ? (
                        filtered.map((ws) => (
                          <RemoteRow
                            key={ws.full_name}
                            workspace={ws}
                            adding={isAddingFullName === ws.full_name}
                            onAdd={() => onAddRemote(ws)}
                          />
                        ))
                      ) : (
                        <div className="px-3 py-6 text-center text-[12px] text-[var(--fg-muted)]">
                          No matching repos.
                        </div>
                      )}
                    </div>
                  </div>
                ) : (
                  <div className="rounded-md border border-dashed border-[var(--line)] px-4 py-6 text-center text-[12.5px] text-[var(--fg-muted)]">
                    Sign in to GitHub from the account menu to browse repos.
                  </div>
                )
              ) : (
                <div className="space-y-3">
                  <div>
                    <div className="text-[11.5px] font-medium text-[var(--fg)]">Repo full name or local path</div>
                    <div className="mt-0.5 text-[11px] text-[var(--fg-muted)]">
                      e.g. <code className="font-mono">acme/team-skills</code> or{" "}
                      <code className="font-mono">~/code/skills</code> or <code className="font-mono">demo</code>
                    </div>
                  </div>
                  <div className="grid grid-cols-[1fr_auto] gap-2">
                    <Input
                      aria-label="Workspace path"
                      value={manualPath}
                      onChange={(event) => setManualPath(event.target.value)}
                      placeholder="acme/team-skills, ~/code/skills, or demo"
                      variant="secondary"
                    />
                    <Button onPress={onAddManual} isPending={manualPending} isDisabled={!manualPath.trim()}>
                      <Plus size={14} />
                      Add
                    </Button>
                  </div>
                </div>
              )}
            </Modal.Body>

            <div className="flex justify-end gap-2 border-t border-[var(--line)] px-5 py-3">
              <Button variant="outline" onPress={() => onOpenChange(false)}>
                Close
              </Button>
            </div>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`relative pb-3 pt-2 text-[12.5px] font-medium ${
        active ? "text-[var(--fg)]" : "text-[var(--fg-muted)] hover:text-[var(--fg-secondary)]"
      }`}
    >
      {children}
      {active ? <span className="absolute -bottom-px left-0 right-0 h-[2px] bg-[var(--brand)]" /> : null}
    </button>
  );
}

function RemoteRow({
  workspace,
  adding,
  onAdd,
}: {
  workspace: Workspace;
  adding: boolean;
  onAdd: () => void;
}) {
  const color = workspaceColor(workspace.full_name);
  const initials = workspaceInitials(workspace);
  return (
    <div className="flex items-center gap-3 border-b border-[var(--line)] px-3 py-2 last:border-b-0">
      <span
        className="grid size-7 place-items-center rounded-md text-[10px] font-semibold"
        style={{ background: color.bg, color: color.fg }}
      >
        {initials}
      </span>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-medium">{workspace.full_name}</div>
        <div className="truncate text-[11px] text-[var(--fg-muted)]">
          {workspace.visibility} · {workspace.permission} · {workspace.default_branch}
        </div>
      </div>
      {workspace.permission === "read" ? <Pill tone="warning">read-only</Pill> : null}
      <Button size="sm" variant="secondary" onPress={onAdd} isPending={adding}>
        <Plus size={13} />
        Add
      </Button>
    </div>
  );
}
