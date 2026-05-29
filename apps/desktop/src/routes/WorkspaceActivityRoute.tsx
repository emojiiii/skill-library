import { useWorkspace } from "../context/WorkspaceContext";
import { ActivityPage } from "../pages/ActivityPage";

export function WorkspaceActivityRoute() {
  const { workspace } = useWorkspace();
  return <ActivityPage workspaceRef={workspace} />;
}
