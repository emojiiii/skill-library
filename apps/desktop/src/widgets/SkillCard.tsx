import { ChevronDown, ChevronRight, Package } from "lucide-react";
import type { SkillAsset } from "../lib/teamai";

export function SkillCard({
  asset,
  selected,
  onSelect,
  expanded,
}: {
  asset: SkillAsset;
  selected: boolean;
  onSelect: () => void;
  expanded?: boolean;
}) {
  return (
    <button type="button" className={`skill-row ${selected ? "selected" : ""}`} onClick={onSelect}>
      {expanded !== undefined ? (
        <span className="skill-row__chevron" aria-hidden="true">
          {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        </span>
      ) : (
        <Package size={14} className="skill-row__icon" />
      )}

      <div className="skill-row__body">
        <div className="skill-row__line1">
          <span className="skill-row__name">{asset.manifest.name}</span>
          {asset.manifest.version ? (
            <span className="skill-row__version">v{asset.manifest.version}</span>
          ) : null}
        </div>
      </div>
    </button>
  );
}
