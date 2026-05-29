import type { ReactNode } from "react";

export function SegmentedTabs<T extends string>({
  tabs,
  active,
  onChange,
  className = "",
}: {
  tabs: Array<{ id: T; label: ReactNode; count?: number }>;
  active: T;
  onChange: (id: T) => void;
  className?: string;
}) {
  return (
    <div className={`segmented-tabs ${className}`}>
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          className={`segmented-tab ${active === tab.id ? "is-active" : ""}`}
          onClick={() => onChange(tab.id)}
        >
          <span>{tab.label}</span>
          {typeof tab.count === "number" ? <span className="segmented-tab__count">{tab.count}</span> : null}
        </button>
      ))}
    </div>
  );
}
