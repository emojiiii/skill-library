import { Button, Input, toast } from "@heroui/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { UseMutationResult } from "@tanstack/react-query";
import { Check, ExternalLink, RefreshCw, UserPlus } from "lucide-react";
import { useLocale } from "../hooks/useLocale";
import type {
  Invitation,
  RepositoryInvitation,
  WorkspaceMember,
} from "../lib/skill-library";
import { acceptRepositoryInvitation, inviteGithubCollaborator, listRepositoryInvitations } from "../lib/skill-library";
import { type InviteRole, inviteRoleLabel, inviteRoles } from "../utils/navigation";
import { formatRelativeTime, openExternalUrl } from "../utils/format";
import { Card } from "../widgets/Card";
import { ManagementTable } from "../widgets/ManagementTable";
import { MetricTile } from "../widgets/MetricTile";
import { Pill } from "../widgets/Pill";
import { MemberRow } from "../widgets/rows";

export function InvitationsPage({
  workspaceRef,
  providerName = "GitHub",
  workspacePermission,
  supportsMembers = true,
  supportsIncomingInvitations = true,
  supportsMemberManagement = true,
  memberManagementBlockedReason = null,
  inviteLogin,
  setInviteLogin,
  inviteRole,
  setInviteRole,
  members,
  membersError,
  inviteCollaborator,
}: {
  workspaceRef: string;
  providerName?: string;
  workspacePermission: string;
  supportsMembers?: boolean;
  supportsIncomingInvitations?: boolean;
  supportsMemberManagement?: boolean;
  memberManagementBlockedReason?: string | null;
  inviteLogin: string;
  setInviteLogin: (value: string) => void;
  inviteRole: InviteRole;
  setInviteRole: (value: InviteRole) => void;
  members: WorkspaceMember[];
  membersError: string | null;
  inviteCollaborator: UseMutationResult<Invitation, Error, void, unknown>;
}) {
  const { t } = useLocale();
  const queryClient = useQueryClient();

  const incoming = useQuery({
    queryKey: ["repo-invitations"],
    queryFn: listRepositoryInvitations,
    staleTime: 60 * 1000,
    enabled: supportsIncomingInvitations,
  });

  const accept = useMutation({
    mutationFn: (invitationId: number) => acceptRepositoryInvitation(invitationId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["repo-invitations"] });
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });

  const incomingList = supportsIncomingInvitations ? incoming.data ?? [] : [];
  const incomingError = supportsIncomingInvitations && incoming.error
    ? incoming.error instanceof Error
      ? incoming.error.message
      : String(incoming.error)
    : null;
  const showMemberManagementBlocked = () => {
    if (memberManagementBlockedReason) toast.warning(memberManagementBlockedReason);
  };

  if (!supportsMembers && !supportsIncomingInvitations && !supportsMemberManagement) {
    return (
      <section className="grid min-h-0 flex-1 place-items-center overflow-hidden px-6 py-6">
        <div className="empty-state mx-auto max-w-md">
          <div className="empty-state__title">{t("invitations.unsupportedTitle")}</div>
          <div>{t("invitations.unsupportedDesc").replace("{provider}", providerName)}</div>
        </div>
      </section>
    );
  }

  return (
    <section className="flex min-h-0 flex-1 overflow-hidden px-6 py-6">
      <div className="mx-auto flex h-full min-h-0 w-full max-w-6xl flex-col gap-5">
        <div className={supportsIncomingInvitations ? "grid gap-3 md:grid-cols-3" : "grid gap-3 md:grid-cols-2"}>
          {supportsIncomingInvitations ? (
            <MetricTile
              label={t("invitations.incoming")}
              value={incomingList.length}
              tone={incomingList.length ? "warning" : "default"}
            />
          ) : null}
          <MetricTile
            label={t("invitations.members")}
            value={members.length}
            tone={members.length ? "success" : "default"}
          />
          <MetricTile label={t("invitations.yourRole")} value={workspacePermission} tone="default" />
        </div>

        <div className="flex min-h-0 flex-1 flex-col gap-5">
          {supportsIncomingInvitations ? (
            <ManagementTable
              title={t("invitations.reposInvitedYou")}
              subtitle={t("invitations.reposInvitedYouSub")}
              count={incomingList.length}
              error={incomingError}
              empty={t("invitations.noInvitations")}
              maxHeightClassName="max-h-[220px]"
              className="shrink-0"
              actions={
                <Button
                  isIconOnly
                  size="sm"
                  variant="tertiary"
                  onPress={() => incoming.refetch()}
                  isPending={incoming.isFetching}
                >
                  <RefreshCw size={13} />
                </Button>
              }
            >
              {incomingList.map((invitation) => (
                <InvitationRow
                  key={invitation.id}
                  invitation={invitation}
                  accepting={accept.isPending && accept.variables === invitation.id}
                  onAccept={() => accept.mutate(invitation.id)}
                />
              ))}
            </ManagementTable>
          ) : null}

          <div className={supportsMemberManagement ? "grid min-h-0 flex-1 gap-5 xl:grid-cols-[400px_minmax(0,1fr)]" : "grid min-h-0 flex-1 gap-5"}>
            {supportsMemberManagement ? (
              <Card className="self-start p-4">
                <div className="mb-3 flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <Card.Title>{t("invitations.inviteCollaborator")}</Card.Title>
                    <Card.Subtitle className="truncate">{workspaceRef || t("invitations.pickWorkspaceFirst")}</Card.Subtitle>
                  </div>
                </div>

                <div className="grid grid-cols-[1fr_auto] gap-2">
                  <Input
                    aria-label={t("invitations.usernamePlaceholder")}
                    name="inviteLogin"
                    value={inviteLogin}
                    onChange={(event) => setInviteLogin(event.target.value)}
                    placeholder={t("invitations.usernamePlaceholder")}
                    variant="secondary"
                    disabled={!workspaceRef}
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                  />
                  <Button
                    onPress={() => {
                      if (memberManagementBlockedReason) {
                        showMemberManagementBlocked();
                        return;
                      }
                      inviteCollaborator.mutate();
                    }}
                    isPending={inviteCollaborator.isPending}
                    isDisabled={!inviteLogin.trim() || !workspaceRef}
                    className={memberManagementBlockedReason ? "opacity-60" : undefined}
                  >
                    <UserPlus size={14} />
                    {t("invitations.invite")}
                  </Button>
                </div>

                <div className="mt-3 flex flex-wrap gap-1.5">
                  {inviteRoles.map((role) => {
                    const active = inviteRole === role;
                    return (
                      <Button
                        key={role}
                        size="sm"
                        variant={active ? "secondary" : "outline"}
                        onPress={() => setInviteRole(role)}
                      >
                        {inviteRoleLabel[role]}
                      </Button>
                    );
                  })}
                </div>

                {inviteCollaborator.data ? (
                  <div className="mt-3 rounded-md border border-[var(--accent)] bg-[var(--accent-soft)] px-3 py-2 text-xs text-[var(--accent-soft-foreground)]">
                    {t("invitations.invited")} {inviteCollaborator.data.login_or_email} · {inviteCollaborator.data.state}
                  </div>
                ) : null}
                {memberManagementBlockedReason ? (
                  <div className="mt-3 rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-xs text-[var(--warning)]">
                    {memberManagementBlockedReason}
                  </div>
                ) : null}
              </Card>
            ) : null}

            {supportsMembers ? (
              <ManagementTable
                title={t("invitations.workspaceMembers")}
                subtitle={t("invitations.workspaceMembersSub")}
                count={members.length}
                error={membersError}
                empty={t("invitations.noCollaborators")}
                className="flex min-h-0 flex-1 flex-col"
                bodyClassName="min-h-0 flex-1"
              >
                {members.map((member) => (
                  <MemberRow
                    key={member.login}
                    member={member}
                    onChangeRole={
                      supportsMemberManagement
                        ? (login, role) => {
                            if (memberManagementBlockedReason) {
                              showMemberManagementBlocked();
                              return;
                            }
                            inviteGithubCollaborator({
                              workspace: workspaceRef,
                              login,
                              role,
                            }).then(() => {
                              queryClient.invalidateQueries({ queryKey: ["workspace-members"] });
                            });
                          }
                        : undefined
                    }
                  />
                ))}
              </ManagementTable>
            ) : null}
          </div>
        </div>
      </div>
    </section>
  );
}

function InvitationRow({
  invitation,
  accepting,
  onAccept,
}: {
  invitation: RepositoryInvitation;
  accepting: boolean;
  onAccept: () => void;
}) {
  const { t } = useLocale();
  return (
    <div className="card-row">
      <div className="min-w-0">
        <div className="flex items-center gap-2 truncate text-[13px] font-medium">
          {invitation.repository_full_name}
          <Pill mono>{invitation.permissions}</Pill>
          {invitation.expired ? <Pill tone="danger">{t("invitations.expired")}</Pill> : null}
        </div>
        <div className="mt-1 flex items-center gap-1.5 text-[11.5px] text-[var(--fg-muted)]">
          <span>{invitation.inviter ? `${t("invitations.invitedBy").replace("{name}", "")}@${invitation.inviter}` : "—"}</span>
          <span>·</span>
          <span>{formatRelativeTime(invitation.created_at)}</span>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          onPress={() => void openExternalUrl(invitation.html_url)}
        >
          <ExternalLink size={12} />
          {t("invitations.view")}
        </Button>
        <Button size="sm" onPress={onAccept} isPending={accepting} isDisabled={invitation.expired}>
          <Check size={12} />
          {t("invitations.accept")}
        </Button>
      </div>
    </div>
  );
}
