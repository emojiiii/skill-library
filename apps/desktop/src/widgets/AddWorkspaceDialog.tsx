import { Button, Input, Label, ListBox, Modal, Select, Spinner } from "@heroui/react";
import { Plus, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { ProviderInstance, Workspace } from "../lib/skill-library";
import {
  providerIsWebDav,
  workspaceKey,
  workspaceProviderLabel,
  workspaceProviderShortLabel,
} from "../lib/providers";
import { workspaceColor, workspaceInitials } from "../utils/workspace-visual";
import { Pill } from "../widgets/Pill";
import { useLocale } from "../hooks/useLocale";

export function AddWorkspaceDialog({
  open,
  onOpenChange,
  remote,
  remoteFetching,
  remoteEnabled,
  providers,
  selectedProviderId,
  onProviderChange,
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
  providers: ProviderInstance[];
  selectedProviderId: string;
  onProviderChange: (providerId: string) => void;
  query: string;
  setQuery: (value: string) => void;
  onAddRemote: (workspace: Workspace) => void;
  isAddingFullName?: string;
  manualPath: string;
  setManualPath: (value: string) => void;
  onAddManual: () => void;
  manualPending: boolean;
}) {
  const { t } = useLocale();
  const [tab, setTab] = useState<"remote" | "manual">("remote");
  const hasProviders = providers.length > 0;
  const selectedProvider = providers.find((provider) => provider.id === selectedProviderId);
  const selectedProviderLabel = hasProviders
    ? selectedProvider?.displayName ?? workspaceProviderLabel(selectedProviderId)
    : t("workspace.add.noProviders.label");
  const isWebDav = hasProviders && providerIsWebDav(selectedProvider);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return remote;
    return remote.filter((ws) => ws.full_name.toLowerCase().includes(needle));
  }, [remote, query]);

  useEffect(() => {
    if (open) {
      setTab(hasProviders && remoteEnabled ? "remote" : "manual");
    }
  }, [hasProviders, open, remoteEnabled]);

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="md">
          <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
            <Modal.CloseTrigger />
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold tracking-tight">{t("workspace.add.title")}</Modal.Heading>
              <div className="mt-1 text-[12px] text-[var(--fg-muted)]">
                {hasProviders
                  ? t("workspace.add.desc").replace("{provider}", selectedProviderLabel)
                  : t("workspace.add.noProviders.desc")}
              </div>
            </Modal.Header>

            <div className="border-b border-[var(--line)] px-5 py-3">
              {hasProviders ? (
                <div className="grid grid-cols-[1fr_auto] items-end gap-2">
                  <Select
                    value={selectedProviderId}
                    onChange={(value) => {
                      if (typeof value === "string" || typeof value === "number") {
                        onProviderChange(String(value));
                      }
                    }}
                    variant="secondary"
                    fullWidth
                    aria-label={t("workspace.add.provider")}
                  >
                    <Label>{t("workspace.add.provider")}</Label>
                    <Select.Trigger>
                      <Select.Value />
                      <Select.Indicator />
                    </Select.Trigger>
                    <Select.Popover>
                      <ListBox>
                        {providers.map((provider) => (
                          <ListBox.Item key={provider.id} id={provider.id} textValue={provider.displayName}>
                            {provider.displayName}
                            <ListBox.ItemIndicator />
                          </ListBox.Item>
                        ))}
                      </ListBox>
                    </Select.Popover>
                  </Select>
                  <span className="workspace-provider-badge workspace-provider-badge--select">
                    {workspaceProviderShortLabel(selectedProviderId)}
                  </span>
                </div>
              ) : (
                <div>
                  <div className="text-[11.5px] font-medium text-[var(--fg)]">
                    {t("workspace.add.provider")}
                  </div>
                  <div className="mt-1.5 rounded-md border border-dashed border-[var(--line)] px-3 py-3 text-[12.5px] text-[var(--fg-muted)]">
                    {t("workspace.add.noProviders.label")}
                  </div>
                </div>
              )}
            </div>

            {hasProviders ? (
              <div className="border-b border-[var(--line)] px-5">
                <div className="flex gap-4">
                  <TabButton active={tab === "remote"} onClick={() => setTab("remote")}>
                    {t("workspace.add.fromProvider")}
                  </TabButton>
                  <TabButton active={tab === "manual"} onClick={() => setTab("manual")}>
                    {t("workspace.add.manual")}
                  </TabButton>
                </div>
              </div>
            ) : null}

            <Modal.Body className="px-5 py-4">
              {!hasProviders ? (
                <div className="rounded-md border border-dashed border-[var(--line)] px-4 py-6 text-center text-[12.5px] text-[var(--fg-muted)]">
                  {t("workspace.add.noProviders.hint")}
                </div>
              ) : tab === "remote" ? (
                remoteEnabled ? (
                  <div className="space-y-3">
                    <div className="relative">
                      <Search size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--fg-muted)]" />
                      <input
                        autoFocus
                        value={query}
                        onChange={(event) => setQuery(event.target.value)}
                        placeholder={t("workspace.add.searchPlaceholder").replace("{provider}", selectedProviderLabel)}
                        className="w-full rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] py-2 pl-8 pr-3 text-[13px] outline-none focus:border-[var(--brand)] focus:ring-2 focus:ring-[var(--brand-soft)]"
                      />
                    </div>
                    <div className="max-h-[320px] overflow-y-auto rounded-md border border-[var(--line)]">
                      {remoteFetching && !remote.length ? (
                        <div className="px-3 py-6 text-center text-[12px] text-[var(--fg-muted)]">{t("common.loading")}</div>
                      ) : filtered.length ? (
                        filtered.map((ws) => (
                          <RemoteRow
                            key={`${ws.provider}:${ws.full_name}`}
                            workspace={ws}
                            adding={isAddingFullName === workspaceKey(ws)}
                            onAdd={() => onAddRemote(ws)}
                          />
                        ))
                      ) : (
                        <div className="px-3 py-6 text-center text-[12px] text-[var(--fg-muted)]">
                          {t("workspace.add.noMatches")}
                        </div>
                      )}
                    </div>
                  </div>
                ) : (
                  <div className="rounded-md border border-dashed border-[var(--line)] px-4 py-6 text-center text-[12.5px] text-[var(--fg-muted)]">
                    {t("workspace.add.signInRequired").replace("{provider}", selectedProviderLabel)}
                  </div>
                )
              ) : (
                <div className="space-y-3">
                  <div>
                    <div className="text-[11.5px] font-medium text-[var(--fg)]">
                      {isWebDav ? t("workspace.add.remoteDirectory") : t("workspace.add.manualLabel")}
                    </div>
                    <div className="mt-0.5 text-[11px] text-[var(--fg-muted)]">
                      {isWebDav
                        ? t("workspace.add.remoteDirectoryHint")
                        : t("workspace.add.manualHint").replace("{provider}", selectedProviderLabel)}
                    </div>
                  </div>
                  <div className="grid grid-cols-[1fr_auto] gap-2">
                    <Input
                      aria-label={t("workspace.add.pathAria")}
                      value={manualPath}
                      onChange={(event) => setManualPath(event.target.value)}
                      placeholder={isWebDav ? t("workspace.add.remoteDirectoryPlaceholder") : t("workspace.add.pathPlaceholder")}
                      variant="secondary"
                    />
                    <Button onPress={onAddManual} isDisabled={!manualPath.trim() || manualPending}>
                      {manualPending ? (
                        <>
                          <Spinner size="sm" />
                          {t("common.adding")}
                        </>
                      ) : (
                        <>
                          <Plus size={14} />
                          {t("common.add")}
                        </>
                      )}
                    </Button>
                  </div>
                </div>
              )}
            </Modal.Body>

            <div className="flex justify-end gap-2 border-t border-[var(--line)] px-5 py-3">
              <Button variant="outline" onPress={() => onOpenChange(false)}>
                {t("common.close")}
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
  const { t } = useLocale();
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
        <div className="flex min-w-0 items-center gap-2">
          <div className="truncate text-[13px] font-medium">{workspace.full_name}</div>
          <span className="workspace-provider-badge is-small">
            {workspaceProviderShortLabel(workspace.provider)}
          </span>
        </div>
        <div className="truncate text-[11px] text-[var(--fg-muted)]">
          {workspace.visibility} · {workspace.permission} · {workspace.default_branch}
        </div>
      </div>
      {workspace.permission === "read" ? <Pill tone="warning">{t("common.readOnly")}</Pill> : null}
      <Button size="sm" variant="secondary" onPress={onAdd} isDisabled={adding}>
        {adding ? (
          <>
            <Spinner size="sm" />
            {t("common.adding")}
          </>
        ) : (
          <>
            <Plus size={13} />
            {t("common.add")}
          </>
        )}
      </Button>
    </div>
  );
}
