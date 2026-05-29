import { useQuery } from "@tanstack/react-query";
import { installSkill, listLocalAgentRoots, listWorkspaces, removeSkill } from "../lib/teamai";
import { LocalPage, type RuntimeKind } from "../pages/LocalPage";
import { formatError } from "../utils/format";
import { useAppStore } from "../state/appStore";

export function InstalledRoute() {
  const setPushEntry = useAppStore((s) => s.setPushEntry);
  const setPushPreview = useAppStore((s) => s.setPushPreview);
  const setPushOpen = useAppStore((s) => s.setPushOpen);

  const localAgents = useQuery({ queryKey: ["local-agents"], queryFn: listLocalAgentRoots, staleTime: 60 * 1000 });
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: listWorkspaces, staleTime: 2 * 60 * 1000 });

  return (
    <LocalPage
      roots={localAgents.data ?? []}
      pending={localAgents.isFetching}
      error={localAgents.error ? formatError(localAgents.error) : null}
      onRefresh={() => localAgents.refetch()}
      onToggleRuntime={(entry, runtime: RuntimeKind, enable) => {
        if (enable) {
          void installSkill(entry.path, [runtime]).then(() => localAgents.refetch());
        } else {
          void removeSkill(entry.id, [runtime]).then(() => localAgents.refetch());
        }
      }}
      onPush={(entry) => {
        setPushEntry(entry);
        setPushPreview(null);
        setPushOpen(true);
      }}
      toggleBusyId={null}
      workspaceCount={workspaces.data?.workspaces.length ?? 0}
    />
  );
}
