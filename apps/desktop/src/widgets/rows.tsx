import { ListBox, Select } from "@heroui/react";
import type {
  InvitationRecord,
  NotificationEvent,
  PublishPolicyCheckRecord,
  PublishRequestRecord,
  WorkspaceMember,
} from "../lib/skill-library";
import { useLocale } from "../hooks/useLocale";
import { formatDateTime, formatRelativeTime, shortHash } from "../utils/format";
import { riskTone, stateTone } from "../utils/risk";
import { Pill } from "./Pill";

export function PublishRequestRow({ request }: { request: PublishRequestRecord }) {
  const { t } = useLocale();
  return (
    <div className="card-row">
      <div className="min-w-0">
        <div className="truncate text-sm font-medium">
          {request.skillId} <span className="text-[var(--muted)] font-normal">v{request.skillVersion}</span>
        </div>
        <div className="mt-1 truncate text-xs text-[var(--muted)]">
          {request.pullRequest
            ? `#${request.pullRequest.number} · ${request.pullRequest.title}`
            : shortHash(request.sourceHash)}
        </div>
        <div className="mt-2 flex flex-wrap gap-1">
          <Pill tone={(riskTone[request.policy.risk_level] ?? "default") as never}>
            {riskTone[request.policy.risk_level] ? t(`risk.level.${request.policy.risk_level}`) : request.policy.risk_level}
          </Pill>
          <Pill tone={request.autoMerged ? "success" : "default"}>
            {request.autoMerged ? t("rows.autoMerged") : t("rows.manual")}
          </Pill>
          <Pill>{formatRelativeTime(request.updatedAt)}</Pill>
        </div>
      </div>
      <div className="flex flex-col items-end gap-1">
        <Pill tone={(stateTone[request.state] ?? "default") as never}>
          {request.state.replace("_", " ")}
        </Pill>
        <Pill tone={request.policy.auto_merge_allowed ? "success" : "warning"}>
          {request.policy.decision.replaceAll("_", " ")}
        </Pill>
      </div>
    </div>
  );
}

export function PolicyCheckRow({ check }: { check: PublishPolicyCheckRecord }) {
  const { t } = useLocale();
  return (
    <div className="card-row">
      <div className="min-w-0">
        <div className="truncate text-sm font-medium">
          {check.skillId ?? t("rows.unknownSkill")}{" "}
          {check.skillVersion ? <span className="text-[var(--muted)] font-normal">v{check.skillVersion}</span> : null}
        </div>
        <div className="mt-1 truncate text-xs text-[var(--muted)]">
          {check.policy.reasons[0] ?? shortHash(check.sourceHash) ?? formatDateTime(check.createdAt)}
        </div>
      </div>
      <div className="flex flex-col items-end gap-1">
        <Pill tone={(riskTone[check.policy.risk_level] ?? "default") as never}>
          {riskTone[check.policy.risk_level] ? t(`risk.level.${check.policy.risk_level}`) : check.policy.risk_level}
        </Pill>
        <Pill tone={check.policy.auto_merge_allowed ? "success" : "warning"}>
          {check.decision.replaceAll("_", " ")}
        </Pill>
      </div>
    </div>
  );
}

export function InvitationRow({ invitation }: { invitation: InvitationRecord }) {
  const { t } = useLocale();
  return (
    <div className="card-row">
      <div className="min-w-0">
        <div className="truncate text-sm font-medium">{invitation.invitee}</div>
        <div className="mt-1 truncate text-xs text-[var(--muted)]">
          {invitation.provider} · {invitation.role} · {invitation.onboardingStatus?.replaceAll("_", " ") ?? t("rows.invited")}
        </div>
      </div>
      <div className="flex flex-col items-end gap-1">
        <Pill tone={(stateTone[invitation.state] ?? "default") as never}>
          {invitation.state}
        </Pill>
        <Pill>{formatRelativeTime(invitation.updatedAt)}</Pill>
      </div>
    </div>
  );
}

export function MemberRow({ member, onChangeRole }: { member: WorkspaceMember; onChangeRole?: (login: string, role: string) => void }) {
  const { t } = useLocale();
  return (
    <div className="card-row">
      <div className="flex min-w-0 items-center gap-3">
        {member.avatar_url ? (
          <img
            src={member.avatar_url}
            alt=""
            className="h-9 w-9 shrink-0 rounded-full border border-[var(--line)] object-cover"
          />
        ) : (
          <div className="grid h-9 w-9 shrink-0 place-items-center rounded-full border border-[var(--line)] bg-[var(--surface-soft)] text-xs font-semibold uppercase text-[var(--muted)]">
            {member.login.slice(0, 2)}
          </div>
        )}
        <div className="min-w-0">
          <div className="truncate text-sm font-medium">@{member.login}</div>
          <div className="mt-0.5 truncate text-xs text-[var(--muted)]">{t("rows.githubCollaborator")}</div>
        </div>
      </div>
      {onChangeRole ? (
        <Select
          value={member.role}
          onChange={(value) => {
            if (typeof value === "string" || typeof value === "number") {
              onChangeRole(member.login, String(value));
            }
          }}
          variant="secondary"
          className="w-[112px]"
          aria-label={t("rows.memberRole")}
        >
          <Select.Trigger>
            <Select.Value />
            <Select.Indicator />
          </Select.Trigger>
          <Select.Popover>
            <ListBox>
              {["read", "triage", "write", "maintain", "admin"].map((role) => (
                <ListBox.Item key={role} id={role} textValue={role}>
                  {role}
                  <ListBox.ItemIndicator />
                </ListBox.Item>
              ))}
            </ListBox>
          </Select.Popover>
        </Select>
      ) : (
        <Pill
          tone={["admin", "maintain"].includes(member.role) ? "success" : member.role === "none" ? "warning" : "default"}
        >
          {member.role}
        </Pill>
      )}
    </div>
  );
}

export function NotificationRow({ notification }: { notification: NotificationEvent }) {
  const { t } = useLocale();
  const ref = notification.ref ?? t("rows.unknownRef");
  const after = notification.after ? shortHash(notification.after) : t("rows.noCommit");

  return (
    <div className="card-row">
      <div className="min-w-0">
        <div className="truncate text-sm font-medium">{notification.repository}</div>
        <div className="mt-1 truncate text-xs text-[var(--muted)]">
          {notification.sourceEvent} · {ref} · {after}
        </div>
        <div className="mt-2 flex flex-wrap gap-1">
          <Pill>{notification.provider}</Pill>
          {notification.delivery ? <Pill>{notification.delivery}</Pill> : null}
        </div>
      </div>
      <div className="flex flex-col items-end gap-1">
        <Pill tone={notification.sourceEvent === "release" ? "success" : "warning"}>
          {notification.kind.replaceAll("_", " ")}
        </Pill>
        <Pill>{formatRelativeTime(notification.receivedAt)}</Pill>
      </div>
    </div>
  );
}
