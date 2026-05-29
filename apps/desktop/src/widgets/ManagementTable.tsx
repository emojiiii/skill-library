import type { ReactNode } from "react";

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
    <div className="card overflow-hidden">
      <div className="card-header">
        <div className="min-w-0">
          <div className="card-title">{title}</div>
          {subtitle ? <div className="card-subtitle">{subtitle}</div> : null}
        </div>
        {actions ? <div className="flex items-center gap-2">{actions}</div> : null}
      </div>
      {error ? (
        <div className="border-b border-[var(--line)] bg-[#fcefe7]/60 px-4 py-2 text-xs text-[var(--status-danger)]">
          {error}
        </div>
      ) : null}
      <div className={`${maxHeightClassName} overflow-y-auto`}>
        {count ? children : <div className="empty-state">{empty}</div>}
      </div>
    </div>
  );
}
