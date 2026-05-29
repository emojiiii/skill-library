import { GitCommit, Tag } from "lucide-react";
import type { SkillAsset, SkillVersion } from "../lib/teamai";
import { useLocale } from "../hooks/useLocale";
import { formatRelativeTime } from "../utils/format";
import { Pill } from "./Pill";

export function ActivityTimeline({
  assets,
  versions,
  onSelectVersion,
}: {
  assets: SkillAsset[];
  versions: SkillVersion[];
  onSelectVersion: (version: string, asset?: SkillAsset) => void;
}) {
  const { t } = useLocale();
  if (!assets.length) {
    return (
      <div className="empty-state">
        <div className="empty-state__title">{t("timeline.noSkills")}</div>
        <div>{t("timeline.noSkillsDesc")}</div>
      </div>
    );
  }
  if (!versions.length) {
    return (
      <div className="empty-state">
        <div className="empty-state__title">{t("timeline.noTags")}</div>
        <div>{t("timeline.noTagsDesc")}</div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {assets.map((asset) => (
        <section key={asset.manifest.id} className="card overflow-hidden">
          <div className="card-header">
            <div>
              <div className="flex items-center gap-2">
                <span className="card-title">{asset.manifest.name}</span>
                <Pill mono>v{asset.manifest.version}</Pill>
              </div>
              <div className="card-subtitle font-mono">{asset.path}</div>
            </div>
            <Pill>{versions.length} refs</Pill>
          </div>
          <div className="px-4 py-3">
            {versions.map((version) => (
              <button
                key={`${asset.manifest.id}:${version.name}`}
                type="button"
                className="timeline-row w-full text-left rounded-md hover:bg-[var(--bg-soft)] px-2"
                onClick={() => onSelectVersion(version.name, asset)}
              >
                <span className="timeline-bullet">
                  {version.name.startsWith("v") ? <Tag size={11} /> : <GitCommit size={11} />}
                </span>
                <div className="min-w-0">
                  <div className="text-[13px] font-medium text-[var(--fg)]">{version.name}</div>
                  <div className="text-[11.5px] font-mono text-[var(--fg-muted)]">
                    {version.sha.slice(0, 12)}
                  </div>
                </div>
                <div className="text-[11px] text-[var(--fg-muted)]">
                  {formatRelativeTime(undefined)}
                </div>
              </button>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
