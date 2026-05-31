import { Tabs } from "@heroui/react";
import type { Key } from "react";
import type { ReactNode } from "react";

/**
 * Thin wrapper over HeroUI `Tabs` used as a pure selector — the tab *content*
 * is rendered by the parent, so we only render Root + List + Tab (no Panel).
 * Keeps the original `tabs`/`active`/`onChange` API so existing call sites stay
 * unchanged.
 *
 * Uses the `secondary` variant: a transparent underline tab bar (bottom border
 * + 2px accent indicator under the active tab) instead of the default filled
 * "segmented control" look, so it sits cleanly on white surfaces. The secondary
 * variant's CSS targets `tabs--secondary > .tabs__list-container > .tabs__list`,
 * so the ListContainer wrapper is required.
 *
 * Note: `Tabs.Indicator` (react-aria's SelectionIndicator) renders a
 * SharedElement that must live inside each `Tabs.Tab` — that's where react-aria
 * provides both the SharedElementTransition and SelectionIndicator contexts.
 * Placing it as a direct child of `Tabs.List` throws
 * "<SharedElement> must be rendered inside a <SharedElementTransition>".
 */
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
    <Tabs
      variant="secondary"
      selectedKey={active}
      onSelectionChange={(key: Key) => onChange(String(key) as T)}
      className={className}
    >
      <Tabs.ListContainer>
        <Tabs.List>
          {tabs.map((tab) => (
            <Tabs.Tab key={tab.id} id={tab.id}>
              <Tabs.Indicator />
              <span className="whitespace-nowrap">{tab.label}</span>
              {typeof tab.count === "number" ? (
                <span className="ml-1 tabular-nums opacity-60">{tab.count}</span>
              ) : null}
            </Tabs.Tab>
          ))}
        </Tabs.List>
      </Tabs.ListContainer>
    </Tabs>
  );
}
