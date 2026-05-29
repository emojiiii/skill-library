import type { ReactNode } from "react";

export type PillTone = "default" | "success" | "warning" | "danger" | "brand";

export function Pill({
  tone = "default",
  mono = false,
  children,
}: {
  tone?: PillTone;
  mono?: boolean;
  children: ReactNode;
}) {
  const cls = ["pill"];
  if (tone !== "default") cls.push(`pill--${tone}`);
  if (mono) cls.push("pill--mono");
  return <span className={cls.join(" ")}>{children}</span>;
}
