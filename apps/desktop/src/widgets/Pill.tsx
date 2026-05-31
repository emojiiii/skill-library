import { Chip } from "@heroui/react";
import type { ReactNode } from "react";

export type PillTone = "default" | "success" | "warning" | "danger" | "brand";

const toneToColor: Record<PillTone, "default" | "success" | "warning" | "danger" | "accent"> = {
  default: "default",
  success: "success",
  warning: "warning",
  danger: "danger",
  brand: "accent",
};

/**
 * Thin wrapper over HeroUI `Chip`. Keeps the original `tone`/`mono` API so the
 * 20+ existing call sites stay unchanged, while rendering a real HeroUI
 * component underneath. `mono` switches the label to a tabular monospace font
 * (used for versions, SHAs, etc.).
 */
export function Pill({
  tone = "default",
  mono = false,
  children,
}: {
  tone?: PillTone;
  mono?: boolean;
  children: ReactNode;
}) {
  return (
    <Chip
      size="sm"
      color={toneToColor[tone]}
      variant="soft"
      className={mono ? "font-mono tabular-nums" : undefined}
    >
      {children}
    </Chip>
  );
}
