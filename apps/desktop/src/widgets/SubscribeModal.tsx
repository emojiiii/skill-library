import { Button, Modal, Switch } from "@heroui/react";
import { useEffect, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import type { SkillAsset } from "../lib/skill-library";
import { Pill } from "../widgets/Pill";

export type UpdatePolicy = "auto-patch" | "auto-minor" | "manual" | "pin";
export type Channel = "stable" | "beta";

const targets = [
  { id: "claude-code", label: "Claude Code", desc: "~/.claude/skills" },
  { id: "codex", label: "Codex", desc: "~/.agents/skills" },
];
const targetIds = new Set(targets.map((target) => target.id));

const policies: Array<{ id: UpdatePolicy; labelKey: string; descKey: string }> = [
  { id: "auto-patch", labelKey: "subscribe.policy.autoPatch", descKey: "subscribe.policy.autoPatch.desc" },
  { id: "auto-minor", labelKey: "subscribe.policy.autoMinor", descKey: "subscribe.policy.autoMinor.desc" },
  { id: "manual", labelKey: "subscribe.policy.manual", descKey: "subscribe.policy.manual.desc" },
  { id: "pin", labelKey: "subscribe.policy.pin", descKey: "subscribe.policy.pin.desc" },
];

function normalizeTargets(values: string[]): string[] {
  return values.filter((value, index) => targetIds.has(value) && values.indexOf(value) === index);
}

export function SubscribeModal({
  open,
  onOpenChange,
  asset,
  workspaceFullName,
  initialTargets,
  onConfirm,
  pending,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  asset: SkillAsset | null;
  workspaceFullName: string;
  initialTargets: string[];
  onConfirm: (input: { targets: string[]; policy: UpdatePolicy; channel: Channel }) => void;
  pending: boolean;
}) {
  const { t } = useLocale();
  const [selected, setSelected] = useState<string[]>(initialTargets);
  const [policy, setPolicy] = useState<UpdatePolicy>("auto-patch");

  useEffect(() => {
    if (open) {
      // Tools default to OFF; selecting none is allowed (downloads without
      // deploying to any tool).
      setSelected(normalizeTargets(initialTargets));
      setPolicy("auto-patch");
    }
  }, [open, initialTargets]);

  if (!asset) return null;

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="md">
          <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
            <Modal.CloseTrigger />
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                {t("subscribe.title")} {asset.manifest.name}
              </Modal.Heading>
              <div className="mt-1 text-[12px] text-[var(--fg-muted)]">
                {workspaceFullName} · v{asset.manifest.version}
              </div>
            </Modal.Header>

            <Modal.Body className="space-y-5 px-5 py-4">
              {/* Targets */}
              <section>
                <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                  {t("subscribe.installTo")}
                </div>
                <div className="space-y-1">
                  {targets.map((target) => {
                    const enabled = selected.includes(target.id);
                    return (
                      <label
                        key={target.id}
                        className="flex cursor-pointer items-center justify-between rounded-md border border-[var(--line)] bg-[var(--bg-elevated)] px-3 py-2 hover:bg-[var(--bg-soft)]"
                      >
                        <div>
                          <div className="text-[13px] font-medium">{target.label}</div>
                          <div className="text-[11.5px] font-mono text-[var(--fg-muted)]">{target.desc}</div>
                        </div>
                        <Switch
                          isSelected={enabled}
                          onChange={(value) =>
                            setSelected((prev) =>
                              value ? Array.from(new Set([...prev, target.id])) : prev.filter((ti) => ti !== target.id),
                            )
                          }
                        >
                          <Switch.Control>
                            <Switch.Thumb />
                          </Switch.Control>
                        </Switch>
                      </label>
                    );
                  })}
                </div>
              </section>

              {/* Update policy */}
              <section>
                <div className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                  {t("subscribe.updatePolicy")}
                </div>
                <div className="grid grid-cols-2 gap-2">
                  {policies.map((p) => {
                    const active = policy === p.id;
                    return (
                      <button
                        key={p.id}
                        type="button"
                        onClick={() => setPolicy(p.id)}
                        className={`rounded-md border px-3 py-2 text-left transition-colors ${
                          active
                            ? "border-[var(--brand)] bg-[var(--brand-soft)]"
                            : "border-[var(--line)] bg-[var(--bg-elevated)] hover:bg-[var(--bg-soft)]"
                        }`}
                      >
                        <div className={`text-[12.5px] font-medium ${active ? "text-[var(--brand-fg)]" : "text-[var(--fg)]"}`}>
                          {t(p.labelKey)}
                        </div>
                        <div className="mt-0.5 text-[11px] text-[var(--fg-muted)]">{t(p.descKey)}</div>
                      </button>
                    );
                  })}
                </div>
              </section>

              {/* Risk preview */}
              {asset.manifest.permissions.length ? (
                <section className="rounded-md border border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2.5">
                  <div className="text-[11px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">
                    {t("subscribe.permissions")}
                  </div>
                  <div className="mt-1.5 flex flex-wrap gap-1">
                    {asset.manifest.permissions.map((perm) => (
                      <Pill key={perm} mono>
                        {perm}
                      </Pill>
                    ))}
                  </div>
                </section>
              ) : null}
            </Modal.Body>

            <div className="flex justify-end gap-2 border-t border-[var(--line)] px-5 py-3">
              <Button variant="outline" onPress={() => onOpenChange(false)}>
                {t("subscribe.cancel")}
              </Button>
              <Button
                onPress={() => onConfirm({ targets: selected, policy, channel: "stable" })}
                isPending={pending}
              >
                {t("subscribe.confirm")}
              </Button>
            </div>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
