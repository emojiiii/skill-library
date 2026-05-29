import { useLocale } from "../hooks/useLocale";
import type { Subscription } from "../lib/teamai";
import { formatRelativeTime } from "../utils/format";
import { MetricTile } from "../widgets/MetricTile";
import { Pill } from "../widgets/Pill";

export function SubscriptionsPage({
  subscriptions,
}: {
  subscriptions: Subscription[];
}) {
  const { t } = useLocale();
  const total = subscriptions.length;
  const autoUpdating = subscriptions.filter((sub) => sub.update.startsWith("auto")).length;
  const pinned = subscriptions.filter((sub) => sub.update === "pin").length;

  return (
    <section className="scroll-area min-h-0 flex-1 px-6 py-6">
      <div className="mx-auto flex max-w-5xl flex-col gap-5">
        <div className="grid gap-3 md:grid-cols-3">
          <MetricTile
            label={t("subscriptions.title")}
            value={total}
            tone={total ? "success" : "default"}
            hint={t("subscriptions.hint.synced")}
          />
          <MetricTile
            label={t("subscriptions.autoUpdating")}
            value={autoUpdating}
            tone={autoUpdating ? "success" : "default"}
            hint="auto-patch / auto-minor"
          />
          <MetricTile
            label={t("subscriptions.pinned")}
            value={pinned}
            tone={pinned ? "warning" : "default"}
            hint={t("subscriptions.hint.manual")}
          />
        </div>

        <div className="card overflow-hidden">
          <div className="card-header">
            <div>
              <div className="card-title">{t("subscriptions.declarations")}</div>
              <div className="card-subtitle">~/.team-ai-hub/subscriptions.yaml</div>
            </div>
            <Pill>{t("subscriptions.entries").replace("{count}", String(total))}</Pill>
          </div>
          {total === 0 ? (
            <div className="empty-state">
              <div className="empty-state__title">{t("subscriptions.empty")}</div>
              <div>{t("subscriptions.empty.desc")}</div>
            </div>
          ) : (
            <div className="divide-y divide-[var(--line)]">
              {subscriptions.map((sub) => {
                const targets: string[] = [];
                if (sub.targets.claude_code) targets.push("claude-code");
                if (sub.targets.cursor) targets.push("cursor");
                if (sub.targets.codex) targets.push("codex");
                targets.push(...sub.targets.custom);
                return (
                  <div
                    key={`${sub.workspace.owner}/${sub.workspace.repo}:${sub.asset_id}`}
                    className="card-row"
                  >
                    <div className="min-w-0">
                      <div className="truncate font-medium">{sub.asset_id}</div>
                      <div className="mt-1 truncate text-xs font-mono text-[var(--muted)]">
                        {sub.workspace.owner}/{sub.workspace.repo} · {sub.channel}
                        {sub.version ? ` · ${sub.version}` : ""}
                      </div>
                      <div className="mt-2 flex flex-wrap gap-1">
                        {targets.map((target) => (
                          <Pill key={target}>{target}</Pill>
                        ))}
                      </div>
                    </div>
                    <div className="flex flex-col items-end gap-1">
                      <Pill
                        tone={sub.update === "pin" ? "warning" : sub.update === "manual" ? "default" : "success"}
                      >
                        {sub.update}
                      </Pill>
                      <Pill>{formatRelativeTime(sub.subscribed_at)}</Pill>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
