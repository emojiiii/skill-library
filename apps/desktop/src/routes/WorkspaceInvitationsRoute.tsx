import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { inviteGithubCollaborator, listWorkspaceMembers } from "../lib/skill-library";
import { InvitationsPage } from "../pages/InvitationsPage";
import { useWorkspace } from "../context/WorkspaceContext";
import { useLocale } from "../hooks/useLocale";
import { formatError } from "../utils/format";
import type { InviteRole } from "../utils/navigation";
import {
  providerIsGitLab,
  providerSupportsInvitations,
  providerSupportsMemberManagement,
  providerSupportsMembersPage,
  workspaceProviderLabel,
} from "../lib/providers";

const MEMBER_MANAGEMENT_ROLES = new Set(["admin", "maintain"]);

export function WorkspaceInvitationsRoute() {
  const { workspace, workspaceMeta, providerId, providerInstance, providerAuthStatus, authLogin } = useWorkspace();
  const { t } = useLocale();
  const [inviteLogin, setInviteLogin] = useState("");
  const [inviteRole, setInviteRole] = useState<InviteRole>("read");
  const supportsMembers = providerSupportsMembersPage(providerInstance ?? undefined, providerId);
  const supportsIncomingInvitations = providerSupportsInvitations(providerInstance ?? undefined, providerId);
  const supportsMemberManagement = providerSupportsMemberManagement(providerInstance ?? undefined, providerId);
  const providerName = providerInstance?.displayName || workspaceProviderLabel(providerId);
  const providerScopes = providerAuthStatus?.scopes?.map((scope) => scope.toLowerCase()) ?? [];
  const workspacePermission = workspaceMeta?.permission?.toLowerCase();
  const authenticated = providerAuthStatus?.authenticated ?? Boolean(authLogin);
  const memberManagementBlockedReason = (() => {
    if (!supportsMemberManagement) return null;
    if (!authenticated) {
      return t("permissions.loginRequired").replace("{provider}", providerName);
    }
    if (
      providerIsGitLab(providerInstance ?? undefined, providerId) &&
      !providerScopes.includes("api")
    ) {
      return t("permissions.gitlabApiScopeRequired").replace("{provider}", providerName);
    }
    if (workspacePermission && !MEMBER_MANAGEMENT_ROLES.has(workspacePermission)) {
      return t("permissions.workspaceMaintainRequired").replace("{role}", workspaceMeta?.permission ?? "-");
    }
    return null;
  })();

  const workspaceMembers = useQuery({
    queryKey: ["workspace-members", workspace, authLogin],
    queryFn: () => listWorkspaceMembers({ workspace }),
    enabled: Boolean(workspace && authLogin && supportsMembers),
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
      providerName={providerName}
      workspacePermission={workspaceMeta?.permission ?? "—"}
      supportsMembers={supportsMembers}
      supportsIncomingInvitations={supportsIncomingInvitations}
      supportsMemberManagement={supportsMemberManagement}
      memberManagementBlockedReason={memberManagementBlockedReason}
      inviteLogin={inviteLogin}
      setInviteLogin={setInviteLogin}
      inviteRole={inviteRole}
      setInviteRole={setInviteRole}
      members={workspaceMembers.data ?? []}
      membersError={workspaceMembers.error ? formatError(workspaceMembers.error) : null}
      inviteCollaborator={inviteCollaborator}
    />
  );
}
