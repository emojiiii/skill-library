import { GitCommit, Tag } from "lucide-react";
import type { SkillAsset, SkillVersion } from "../lib/skill-library";
import { useLocale } from "../hooks/useLocale";
import { formatRelativeTime } from "../utils/format";
import { Card } from "./Card";
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
        <Card key={asset.manifest.id} className="overflow-hidden p-0 gap-0">
          <Card.Header>
            <div>
              <div className="flex items-center gap-2">
                <span className="text-[13px] font-semibold text-[var(--fg)]">{asset.manifest.name}</span>
                <Pill mono>v{asset.manifest.version}</Pill>
              </div>
              <Card.Subtitle className="font-mono">{asset.path}</Card.Subtitle>
            </div>
            <Pill>{versions.length} refs</Pill>
          </Card.Header>
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
        </Card>
      ))}
    </div>
  );
}
