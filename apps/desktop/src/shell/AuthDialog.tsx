import { Button, Input, Modal } from "@heroui/react";
import { Github, KeyRound, ShieldAlert } from "lucide-react";
import { useState } from "react";
import { useLocale } from "../hooks/useLocale";
import type { GitHubDeviceStartResult } from "../lib/skill-library";
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
        <Modal.Container size="sm">
          <Modal.Dialog className="auth-dialog">
            <Modal.CloseTrigger />
            <Modal.Header className="auth-dialog__header">
              <div className="auth-dialog__identity">
                <div className="auth-dialog__mark">
                  <Github size={18} />
                </div>
                <div className="min-w-0">
                  <Modal.Heading className="auth-dialog__title">
                    {t("auth.account")}
                  </Modal.Heading>
                  <div className="auth-dialog__subtitle">
                    {authLogin ? t("auth.signedInAs").replace("{login}", authLogin) : t("auth.connectGithub")}
                  </div>
                </div>
              </div>
            </Modal.Header>

            <Modal.Body className="auth-dialog__body">
              {authLogin ? (
                <div className="auth-dialog__account-card">
                  <div className="flex items-center gap-2.5">
                    <span className="auth-dialog__avatar">
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
                    <div className="auth-dialog__intent">
                      {intentReason}
                    </div>
                  ) : null}
                  <Button
                    fullWidth
                    className="auth-dialog__primary"
                    onPress={onStartDevice}
                    isPending={startPending || pollPending}
                  >
                    <KeyRound size={15} />
                    {t("auth.continueWithGithub")}
                  </Button>

                  {device ? <DeviceCodePanel device={device} status={deviceStatus} /> : null}

                  <div className="auth-dialog__token-section">
                    <button
                      type="button"
                      onClick={() => setShowTokenForm((value) => !value)}
                      className="auth-dialog__token-trigger"
                    >
                      <ShieldAlert size={13} />
                      {t("auth.useToken")}
                    </button>
                    {showTokenForm ? (
                      <div className="auth-dialog__token-panel">
                        <div className="grid grid-cols-[1fr_auto] gap-2">
                          <Input
                            aria-label={t("auth.githubTokenAria")}
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
                <div className="auth-dialog__error">
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
