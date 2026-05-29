import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { inviteGithubCollaborator, listWorkspaceMembers } from "../lib/teamai";
import { InvitationsPage } from "../pages/InvitationsPage";
import { useWorkspace } from "../context/WorkspaceContext";
import { formatError } from "../utils/format";
import type { InviteRole } from "../utils/navigation";

export function WorkspaceInvitationsRoute() {
  const { workspace, workspaceMeta, authLogin } = useWorkspace();
  const [inviteLogin, setInviteLogin] = useState("");
  const [inviteRole, setInviteRole] = useState<InviteRole>("read");

  const workspaceMembers = useQuery({
    queryKey: ["workspace-members", workspace, authLogin],
    queryFn: () => listWorkspaceMembers({ workspace }),
    enabled: Boolean(workspace && authLogin),
  });

  const inviteCollaborator = useMutation({
    mutationFn: () =>
      inviteGithubCollaborator({
        workspace,
        login: inviteLogin.trim(),
        role: inviteRole,
      }),
    onSuccess: () => {
      setInviteLogin("");
      workspaceMembers.refetch();
    },
  });

  return (
    <InvitationsPage
      workspaceRef={workspace}
      workspacePermission={workspaceMeta?.permission ?? "—"}
      inviteLogin={inviteLogin}
      setInviteLogin={setInviteLogin}
      inviteRole={inviteRole}
      setInviteRole={setInviteRole}
      invitations={[]}
      invitationError={null}
      members={workspaceMembers.data ?? []}
      membersError={workspaceMembers.error ? formatError(workspaceMembers.error) : null}
      inviteCollaborator={inviteCollaborator}
    />
  );
}
