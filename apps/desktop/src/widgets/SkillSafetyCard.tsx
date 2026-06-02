import { AlertTriangle, ShieldAlert, ShieldCheck } from "lucide-react";
import type { SkillManifest } from "../lib/skill-library";
import { useLocale } from "../hooks/useLocale";
import { plainPermissionLines, safetyLevel, type SafetyLevel } from "../utils/risk";

const ICON: Record<SafetyLevel, typeof ShieldCheck> = {
  safe: ShieldCheck,
  caution: AlertTriangle,
  sensitive: ShieldAlert,
};

const ACCENT: Record<SafetyLevel, string> = {
  safe: "var(--success)",
  caution: "var(--warning)",
  sensitive: "var(--danger)",
};

const SOFT: Record<SafetyLevel, string> = {
  safe: "var(--success-soft)",
  caution: "var(--warning-soft)",
  sensitive: "var(--danger-soft)",
};

/**
 * The one piece of "risk" UI a non-technical user must see. Translates the
 * developer risk model into plain language: a single headline (safe / be
 * careful / sensitive) plus what the skill can actually do to their computer.
 */
export function SkillSafetyCard({ manifest }: { manifest: SkillManifest }) {
  const { t } = useLocale();
  const level = safetyLevel(manifest);
  const Icon = ICON[level];
  const caps = plainPermissionLines(manifest, t);

  return (
    <div
      className="rounded-[10px] border px-4 py-3"
      style={{ borderColor: ACCENT[level], background: SOFT[level] }}
    >
      <div className="flex items-center gap-2">
        <Icon size={16} style={{ color: ACCENT[level] }} />
        <span className="text-[13.5px] font-semibold" style={{ color: ACCENT[level] }}>
          {t(`safety.headline.${level}`)}
        </span>
      </div>
      <div className="mt-1.5 text-[12.5px] leading-[1.55] text-[var(--fg-secondary)]">
        {caps.length ? (
          <ul className="space-y-0.5">
            {caps.map((line) => (
              <li key={line} className="flex items-start gap-1.5">
                <span className="mt-[7px] size-1 shrink-0 rounded-full" style={{ background: ACCENT[level] }} />
                <span>{line}</span>
              </li>
            ))}
          </ul>
        ) : (
          <span>{t("safety.readonly")}</span>
        )}
      </div>
    </div>
  );
}
