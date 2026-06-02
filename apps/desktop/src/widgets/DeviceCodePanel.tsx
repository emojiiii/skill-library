import { Button } from "@heroui/react";
import { Check, Copy, ExternalLink } from "lucide-react";
import { useEffect, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import type { GitHubDeviceStartResult } from "../lib/skill-library";
import { openExternalUrl } from "../utils/format";
import { Pill } from "./Pill";

export function DeviceCodePanel({
  device,
  status,
}: {
  device: GitHubDeviceStartResult;
  status: string;
}) {
  const { t } = useLocale();
  const [copiedCode, setCopiedCode] = useState(false);
  const [copiedUrl, setCopiedUrl] = useState(false);
  const verifyUrl = device.verificationUriComplete ?? device.verificationUri;
  const baseUrl = device.verificationUri;

  useEffect(() => {
    if (copiedCode) {
      const t = window.setTimeout(() => setCopiedCode(false), 1600);
      return () => window.clearTimeout(t);
    }
  }, [copiedCode]);

  useEffect(() => {
    if (copiedUrl) {
      const t = window.setTimeout(() => setCopiedUrl(false), 1600);
      return () => window.clearTimeout(t);
    }
  }, [copiedUrl]);

  const copy = async (value: string, mark: (b: boolean) => void) => {
    try {
      await navigator.clipboard.writeText(value);
      mark(true);
    } catch {
      mark(false);
    }
  };

  return (
    <div className="device-code">
      <div className="device-code__steps">
        <div className="device-code__step">
          <span className="device-code__step-num">1</span>
          <span className="device-code__step-label">{t("device.step1")}</span>
        </div>
        <div className="device-code__code-row">
          <div className="device-code__code">{device.userCode}</div>
          <Button
            size="sm"
            variant={copiedCode ? "secondary" : "outline"}
            onPress={() => copy(device.userCode, setCopiedCode)}
          >
            {copiedCode ? <Check size={13} /> : <Copy size={13} />}
            {copiedCode ? t("device.copied") : t("device.copy")}
          </Button>
        </div>
      </div>

      <div className="device-code__divider" />

      <div className="device-code__steps">
        <div className="device-code__step">
          <span className="device-code__step-num">2</span>
          <span className="device-code__step-label">{t("device.step2")}</span>
        </div>
        <div className="device-code__url-row">
          <code className="device-code__url" title={baseUrl}>
            {baseUrl}
          </code>
          <div className="device-code__url-actions">
            <Button
              size="sm"
              variant={copiedUrl ? "secondary" : "outline"}
              onPress={() => copy(verifyUrl, setCopiedUrl)}
            >
              {copiedUrl ? <Check size={13} /> : <Copy size={13} />}
              {copiedUrl ? t("device.copied") : t("device.copyLink")}
            </Button>
            <Button size="sm" onPress={() => void openExternalUrl(verifyUrl)}>
              <ExternalLink size={13} />
              {t("device.open")}
            </Button>
          </div>
        </div>
      </div>

      <div className="device-code__footer">
        <span className="device-code__pulse" />
        <span className="device-code__status">{status}</span>
        {device.scopes.length ? (
          <span className="ml-auto flex flex-wrap gap-1">
            {device.scopes.map((scope) => (
              <Pill key={scope} mono>
                {scope}
              </Pill>
            ))}
          </span>
        ) : null}
      </div>
    </div>
  );
}
