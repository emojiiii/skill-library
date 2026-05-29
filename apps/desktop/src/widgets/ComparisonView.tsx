import type { SemanticChange, SkillComparison } from "../lib/teamai";
import { riskLabel, riskTone } from "../utils/risk";
import { Pill } from "./Pill";

function formatChangeValue(value: unknown) {
  if (value === null || value === undefined) return "none";
  if (typeof value === "string") return value;
  return JSON.stringify(value);
}

function SemanticChangeRow({ change }: { change: SemanticChange }) {
  const risk = change.risk ?? undefined;
  return (
    <div className="rounded-md border border-[var(--line)] bg-[var(--surface)] p-3 text-sm">
      <div className="mb-2 flex items-center justify-between gap-2">
        <span className="font-medium font-mono text-[12.5px]">{change.path}</span>
        <div className="flex gap-1">
          <Pill tone={change.kind === "added" ? "warning" : change.kind === "removed" ? "danger" : "default"}>
            {change.kind}
          </Pill>
          {risk ? (
            <Pill tone={(riskTone[risk] ?? "default") as never}>
              {riskLabel[risk]}
            </Pill>
          ) : null}
        </div>
      </div>
      <div className="grid grid-cols-2 gap-2 text-xs text-[var(--muted)]">
        <code className="semantic-value">{formatChangeValue(change.before ?? change.value ?? null)}</code>
        <code className="semantic-value">{formatChangeValue(change.after ?? change.value ?? null)}</code>
      </div>
    </div>
  );
}

export function ComparisonView({ comparison }: { comparison: SkillComparison }) {
  return (
    <div className="space-y-3">
      <div className="card overflow-hidden">
        <div className="card-header">
          <div>
            <div className="card-title">Manifest changes</div>
            <div className="card-subtitle">
              {comparison.from} → {comparison.to}
            </div>
          </div>
          <Pill tone={comparison.semantic.length ? "warning" : "success"}>
            {comparison.semantic.length} changes
          </Pill>
        </div>
        <div className="card-body">
          {comparison.semantic.length ? (
            <div className="space-y-2">
              {comparison.semantic.map((change, index) => (
                <SemanticChangeRow key={`${change.path}:${index}`} change={change} />
              ))}
            </div>
          ) : (
            <div className="text-sm text-[var(--muted)]">No manifest-level changes.</div>
          )}
        </div>
      </div>

      <div className="card overflow-hidden">
        <div className="card-header">
          <div className="min-w-0">
            <div className="card-title">File patches</div>
            <div className="card-subtitle truncate">{comparison.skillPath}</div>
          </div>
          <Pill tone={comparison.files.length ? "default" : "success"}>
            {comparison.files.length} files
          </Pill>
        </div>
        <div className="card-body">
          {comparison.files.length ? (
            <div className="space-y-3">
              {comparison.files.map((file) => (
                <div key={file.filename} className="rounded-md border border-[var(--line)] bg-[var(--surface)]">
                  <div className="flex items-center justify-between border-b border-[var(--line)] px-3 py-2 text-sm">
                    <span className="font-mono text-[12.5px]">{file.filename}</span>
                    <Pill tone={file.status === "modified" ? "warning" : "default"}>
                      {file.status}
                    </Pill>
                  </div>
                  <pre className="diff-panel">{file.patch ?? "(no textual patch returned)"}</pre>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-sm text-[var(--muted)]">No file patches were returned for this skill path.</div>
          )}
        </div>
      </div>
    </div>
  );
}
