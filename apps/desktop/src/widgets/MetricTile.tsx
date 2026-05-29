export type MetricTone = "success" | "warning" | "danger" | "default";

const toneAccent: Record<MetricTone, string> = {
  success: "var(--success)",
  warning: "var(--warning)",
  danger: "var(--danger)",
  default: "var(--fg-muted)",
};

export function MetricTile({
  label,
  value,
  tone,
  hint,
}: {
  label: string;
  value: number | string;
  tone: MetricTone;
  hint?: string;
}) {
  return (
    <div className="card p-4">
      <div className="text-[10.5px] font-semibold uppercase tracking-wider text-[var(--fg-muted)]">{label}</div>
      <div
        className="mt-2 text-[26px] font-semibold leading-none tracking-tight tabular-nums"
        style={{ color: toneAccent[tone] }}
      >
        {value}
      </div>
      {hint ? <div className="mt-2 text-[11.5px] text-[var(--fg-muted)]">{hint}</div> : null}
    </div>
  );
}
