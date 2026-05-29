import { Button, Input } from "@heroui/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { UseMutationResult } from "@tanstack/react-query";
import { Check, ExternalLink, RefreshCw, UserPlus } from "lucide-react";
import { useLocale } from "../hooks/useLocale";
import type {
  Invitation,
  InvitationRecord,
  RepositoryInvitation,
  WorkspaceMember,
} from "../lib/teamai";
import { acceptRepositoryInvitation, inviteGithubCollaborator, listRepositoryInvitations } from "../lib/teamai";
import { type InviteRole, inviteRoleLabel, inviteRoles } from "../utils/navigation";
import { formatRelativeTime, openExternalUrl } from "../utils/format";
import { ManagementTable } from "../widgets/ManagementTable";
import { MetricTile } from "../widgets/MetricTile";
import { Pill } from "../widgets/Pill";
import { MemberRow } from "../widgets/rows";

export function InvitationsPage({
  workspaceRef,
  workspacePermission,
  inviteLogin,
  setInviteLogin,
  inviteRole,
  setInviteRole,
  members,
  membersError,
  inviteCollaborator,
}: {
  workspaceRef: string;
  workspacePermission: string;
  inviteLogin: string;
  setInviteLogin: (value: string) => void;
  inviteRole: InviteRole;
  setInviteRole: (value: InviteRole) => void;
  invitations?: InvitationRecord[];
  invitationError?: string | null;
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
  });

  const accept = useMutation({
    mutationFn: (invitationId: number) => acceptRepositoryInvitation(invitationId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["repo-invitations"] });
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });

  const incomingList = incoming.data ?? [];
  const incomingError = incoming.error
    ? incoming.error instanceof Error
      ? incoming.error.message
      : String(incoming.error)
    : null;

  return (
    <section className="scroll-area min-h-0 flex-1 px-6 py-6">
      <div className="mx-auto flex max-w-6xl flex-col gap-5">
        <div className="grid gap-3 md:grid-cols-3">
          <MetricTile
            label={t("invitations.incoming")}
            value={incomingList.length}
            tone={incomingList.length ? "warning" : "default"}
          />
          <MetricTile
            label={t("invitations.members")}
            value={members.length}
            tone={members.length ? "success" : "default"}
          />
          <MetricTile label={t("invitations.yourRole")} value={workspacePermission} tone="default" />
        </div>

        <ManagementTable
          title={t("invitations.reposInvitedYou")}
          subtitle={t("invitations.reposInvitedYouSub")}
          count={incomingList.length}
          error={incomingError}
          empty={t("invitations.noInvitations")}
          maxHeightClassName="max-h-[360px]"
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

        <div className="grid gap-5 xl:grid-cols-[400px_minmax(0,1fr)]">
          <div className="card p-4">
            <div className="mb-3 flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="card-title">{t("invitations.inviteCollaborator")}</div>
                <div className="card-subtitle truncate">{workspaceRef || t("invitations.pickWorkspaceFirst")}</div>
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
                onPress={() => inviteCollaborator.mutate()}
                isPending={inviteCollaborator.isPending}
                isDisabled={!inviteLogin.trim() || !workspaceRef}
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
              <div className="mt-3 rounded-md border border-[var(--accent)] bg-[var(--accent-soft)] px-3 py-2 text-xs text-[var(--accent-fg)]">
                {t("invitations.invited")} {inviteCollaborator.data.login_or_email} · {inviteCollaborator.data.state}
              </div>
            ) : null}
          </div>

          <ManagementTable
            title={t("invitations.workspaceMembers")}
            subtitle={t("invitations.workspaceMembersSub")}
            count={members.length}
            error={membersError}
            empty={t("invitations.noCollaborators")}
            maxHeightClassName="max-h-[420px]"
          >
            {members.map((member) => (
              <MemberRow
                key={member.login}
                member={member}
                onChangeRole={(login, role) => {
                  // Use the invite API to update permission (GitHub treats re-invite as permission update)
                  inviteGithubCollaborator({
                    workspace: workspaceRef,
                    login,
                    role,
                  }).then(() => {
                    queryClient.invalidateQueries({ queryKey: ["workspace-members"] });
                  });
                }}
              />
            ))}
          </ManagementTable>
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
