import { Button, Input } from "@heroui/react";
import { Box, KeyRound, ShieldAlert } from "lucide-react";
import { useState } from "react";
import { useLocale } from "../hooks/useLocale";
import type { GitHubDeviceStartResult } from "../lib/teamai";
import { isTauri } from "../lib/teamai";
import { DeviceCodePanel } from "../widgets/DeviceCodePanel";
import { Pill } from "../widgets/Pill";

export function LoginScreen({
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
  authScopes,
  authWarning,
}: {
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
  authScopes: string[];
  authWarning: string | null | undefined;
}) {
  const { t } = useLocale();
  const [showTokenForm, setShowTokenForm] = useState(false);
  const errors = [startError, pollError, saveError].filter(Boolean) as string[];

  return (
    <div className="login-screen">
      <div className="login-card">
        <div className="flex items-center gap-3">
          <div className="grid size-10 place-items-center rounded-[10px] bg-[#0f1115] text-white">
            <Box size={18} />
          </div>
          <div>
            <div className="text-[16px] font-semibold tracking-tight">{t("login.title")}</div>
            <div className="text-[12px] text-[var(--fg-muted)]">{t("login.subtitle")}</div>
          </div>
        </div>

        <h1 className="mt-6 text-[20px] font-semibold tracking-tight text-[var(--fg)]">{t("login.heading")}</h1>
        <p className="mt-1 text-[13px] text-[var(--fg-muted)]">
          {t("login.description")}
        </p>

        {!isTauri ? (
          <div className="mt-4 rounded-md border border-[var(--warning)] bg-[var(--warning-soft)] px-3 py-2 text-[12px] text-[var(--warning)]">
            <strong>{t("login.browserWarning")}</strong> {t("login.browserWarningDesc")}{" "}
            <code className="font-mono">pnpm tauri dev</code> {t("login.browserWarningEnd")}
          </div>
        ) : null}

        <Button
          fullWidth
          className="mt-5 h-11"
          onPress={onStartDevice}
          isPending={startPending || pollPending}
        >
          <KeyRound size={15} />
          {t("login.continueWithGithub")}
        </Button>

        {device ? <DeviceCodePanel device={device} status={deviceStatus} /> : null}

        <button
          type="button"
          onClick={() => setShowTokenForm((value) => !value)}
          className="mt-3 flex items-center gap-2 text-[12px] font-medium text-[var(--fg-secondary)] hover:text-[var(--fg)]"
        >
          <ShieldAlert size={13} />
          {t("login.useToken")}
        </button>
        {showTokenForm ? (
          <div className="mt-2 rounded-[10px] border border-[var(--line)] bg-[var(--bg-elevated)] p-3">
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
                {t("login.save")}
              </Button>
            </div>
            {authScopes.length ? (
              <div className="mt-2 flex flex-wrap gap-1">
                {authScopes.map((scope) => (
                  <Pill key={scope} mono>
                    {scope}
                  </Pill>
                ))}
              </div>
            ) : null}
            {authWarning ? <div className="mt-2 text-[11px] text-[var(--fg-muted)]">{authWarning}</div> : null}
          </div>
        ) : null}

        {errors.length ? (
          <div className="mt-3 rounded-md border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-[12px] text-[var(--danger)]">
            {errors.map((error, idx) => (
              <div key={`${idx}:${error}`}>{error}</div>
            ))}
          </div>
        ) : null}

        <p className="mt-4 text-[11px] leading-[1.5] text-[var(--fg-muted)]">
          {t("login.tokenHint")}
        </p>
      </div>
    </div>
  );
}
