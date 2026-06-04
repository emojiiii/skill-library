import { useWorkspace } from "../context/WorkspaceContext";
import { useLocale } from "../hooks/useLocale";
import {
  providerIsGitLab,
  providerSupportsPullRequestActions,
  providerSupportsPullRequestPage,
  workspaceProviderLabel,
} from "../lib/providers";
import { PublishPage } from "../pages/PublishPage";

const WRITE_ROLES = new Set(["admin", "maintain", "write"]);

export function WorkspacePublishRoute() {
  const { workspace, workspaceMeta, providerId, providerInstance, providerAuthStatus, authLogin } = useWorkspace();
  const { t } = useLocale();
  const providerName = providerInstance?.displayName || workspaceProviderLabel(providerId);
  const providerScopes = providerAuthStatus?.scopes?.map((scope) => scope.toLowerCase()) ?? [];
  const workspacePermission = workspaceMeta?.permission?.toLowerCase();
  const authenticated = providerAuthStatus?.authenticated ?? Boolean(authLogin);
  const supportsPullRequestActions = providerSupportsPullRequestActions(providerInstance ?? undefined, providerId);
  const pullRequestActionBlockedReason = (() => {
    if (!supportsPullRequestActions) return null;
    if (!authenticated) {
      return t("permissions.loginRequired").replace("{provider}", providerName);
    }
    if (
      providerIsGitLab(providerInstance ?? undefined, providerId) &&
      !providerScopes.includes("api")
    ) {
      return t("permissions.gitlabApiScopeRequired").replace("{provider}", providerName);
    }
    if (workspacePermission && !WRITE_ROLES.has(workspacePermission)) {
      return t("permissions.workspaceWriteRequired").replace("{role}", workspaceMeta?.permission ?? "-");
    }
    return null;
  })();

  return (
    <PublishPage
      workspaceRef={workspace}
      providerName={providerName}
      supportsPullRequests={providerSupportsPullRequestPage(providerInstance ?? undefined, providerId)}
      supportsPullRequestActions={supportsPullRequestActions}
      pullRequestActionBlockedReason={pullRequestActionBlockedReason}
    />
  );
}
