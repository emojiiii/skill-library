import { useWorkspace } from "../context/WorkspaceContext";
import { PublishPage } from "../pages/PublishPage";

export function WorkspacePublishRoute() {
  const { workspace } = useWorkspace();
  return <PublishPage workspaceRef={workspace} />;
}
