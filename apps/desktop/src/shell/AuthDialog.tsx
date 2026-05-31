import { Button, Input, Modal } from "@heroui/react";
import { Box, KeyRound, ShieldAlert, X } from "lucide-react";
import { useState } from "react";
import { useLocale } from "../hooks/useLocale";
import type { GitHubDeviceStartResult } from "../lib/teamai";
import { DeviceCodePanel } from "../widgets/DeviceCodePanel";
import { Pill } from "../widgets/Pill";

export function AuthDialog({
  open,
  onOpenChange,
  authLogin,
  authScopes,
  authWarning,
  intentReason,
  onStartDevice,
  startPending,
  startError,
  device,
  deviceStatus,
  pollPending,
  pollError,
  githubToken,
  setGithubToken,
  onSaveToken,
  savePending,
  saveError,
  onSignOut,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  authLogin: string | null | undefined;
  authScopes: string[];
  authWarning: string | null | undefined;
  intentReason?: string;
  onStartDevice: () => void;
  startPending: boolean;
  startError?: string | null;
  device: GitHubDeviceStartResult | null;
  deviceStatus: string;
  pollPending: boolean;
  pollError?: string | null;
  githubToken: string;
  setGithubToken: (value: string) => void;
  onSaveToken: () => void;
  savePending: boolean;
  saveError?: string | null;
  onSignOut?: () => void;
}) {
  const { t } = useLocale();
  const [showTokenForm, setShowTokenForm] = useState(false);
  const errors = [startError, pollError, saveError].filter(Boolean) as string[];

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="md">
          <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
            <Modal.Header className="flex items-start justify-between gap-3 border-b border-[var(--line)] px-5 py-4">
              <div className="flex items-center gap-3">
                <div className="grid size-9 place-items-center rounded-[10px] bg-[#0f1115] text-white">
                  <Box size={16} />
                </div>
                <div>
                  <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                    {t("auth.account")}
                  </Modal.Heading>
                  <div className="mt-0.5 text-[12px] text-[var(--fg-muted)]">
                    {authLogin ? t("auth.signedInAs").replace("{login}", authLogin) : t("auth.connectGithub")}
                  </div>
                </div>
              </div>
              <button
                type="button"
                onClick={() => onOpenChange(false)}
                className="rounded-md p-1 text-[var(--fg-muted)] hover:bg-[var(--bg-soft)]"
              >
                <X size={14} />
              </button>
            </Modal.Header>

            <Modal.Body className="space-y-4 px-5 py-4">
              {authLogin ? (
                <div className="flex items-center justify-between gap-3 rounded-md border border-[var(--line)] bg-[var(--bg-soft)] px-3 py-2.5">
                  <div className="flex items-center gap-2.5">
                    <span className="grid size-8 place-items-center rounded-full bg-[var(--brand-soft)] text-[11px] font-semibold text-[var(--brand-fg)]">
                      {authLogin.slice(0, 2).toUpperCase()}
                    </span>
                    <div>
                      <div className="text-[13px] font-medium">@{authLogin}</div>
                      {authScopes.length ? (
                        <div className="mt-0.5 flex flex-wrap gap-1">
                          {authScopes.map((scope) => (
                            <Pill key={scope} mono>
                              {scope}
                            </Pill>
                          ))}
                        </div>
                      ) : null}
                    </div>
                  </div>
                  {onSignOut ? (
                    <Button size="sm" variant="outline" onPress={onSignOut}>
                      {t("auth.signOut")}
                    </Button>
                  ) : null}
                </div>
              ) : (
                <>
                  {intentReason ? (
                    <div className="rounded-md border border-[var(--brand)] bg-[var(--brand-soft)] px-3 py-2.5 text-[12.5px] text-[var(--brand-fg)]">
                      {intentReason}
                    </div>
                  ) : null}
                  <Button
                    fullWidth
                    className="h-11"
                    onPress={onStartDevice}
                    isPending={startPending || pollPending}
                  >
                    <KeyRound size={15} />
                    {t("auth.continueWithGithub")}
                  </Button>

                  {device ? <DeviceCodePanel device={device} status={deviceStatus} /> : null}

                  <div>
                    <button
                      type="button"
                      onClick={() => setShowTokenForm((value) => !value)}
                      className="flex items-center gap-2 text-[12px] font-medium text-[var(--fg-secondary)] hover:text-[var(--fg)]"
                    >
                      <ShieldAlert size={13} />
                      {t("auth.useToken")}
                    </button>
                    {showTokenForm ? (
                      <div className="mt-3 rounded-md border border-[var(--line)] p-3">
                        <div className="grid grid-cols-[1fr_auto] gap-2">
                          <Input
                            aria-label="GitHub token"
                            name="githubToken"
                            type="password"
                            value={githubToken}
                            onChange={(event) => setGithubToken(event.target.value)}
                            placeholder="ghp_…"
                            variant="secondary"
                          />
                          <Button
                            variant="secondary"
                            onPress={onSaveToken}
                            isPending={savePending}
                            isDisabled={!githubToken.trim()}
                          >
                            {t("auth.save")}
                          </Button>
                        </div>
                        {authWarning ? (
                          <div className="mt-2 text-[11px] text-[var(--fg-muted)]">{authWarning}</div>
                        ) : null}
                      </div>
                    ) : null}
                  </div>
                </>
              )}

              {errors.length ? (
                <div className="rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
                  {errors.map((error, idx) => (
                    <div key={`${idx}:${error}`}>{error}</div>
                  ))}
                </div>
              ) : null}
            </Modal.Body>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
