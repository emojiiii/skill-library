import type { ReactNode } from "react";
import { Card } from "./Card";

export function ManagementTable({
  title,
  subtitle,
  count,
  error,
  empty,
  maxHeightClassName = "max-h-[480px]",
  children,
  actions,
}: {
  title: string;
  subtitle?: string;
  count: number;
  error: string | null;
  empty: string;
  maxHeightClassName?: string;
  children: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <Card className="overflow-hidden p-0 gap-0">
      <Card.Header>
        <div className="min-w-0">
          <Card.Title>{title}</Card.Title>
          {subtitle ? <Card.Subtitle>{subtitle}</Card.Subtitle> : null}
        </div>
        {actions ? <div className="flex items-center gap-2">{actions}</div> : null}
      </Card.Header>
      {error ? (
        <div className="border-b border-[var(--line)] bg-[#fcefe7]/60 px-4 py-2 text-xs text-[var(--status-danger)]">
          {error}
        </div>
      ) : null}
      <div className={`${maxHeightClassName} overflow-y-auto`}>
        {count ? children : <div className="empty-state">{empty}</div>}
      </div>
    </Card>
  );
}
