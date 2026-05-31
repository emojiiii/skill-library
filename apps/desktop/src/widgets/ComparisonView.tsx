import type { SemanticChange, SkillComparison } from "../lib/teamai";
import { riskLabel, riskTone } from "../utils/risk";
import { Card } from "./Card";
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
      <Card className="overflow-hidden p-0 gap-0">
        <Card.Header>
          <div>
            <Card.Title>Manifest changes</Card.Title>
            <Card.Subtitle>
              {comparison.from} → {comparison.to}
            </Card.Subtitle>
          </div>
          <Pill tone={comparison.semantic.length ? "warning" : "success"}>
            {comparison.semantic.length} changes
          </Pill>
        </Card.Header>
        <Card.Body>
          {comparison.semantic.length ? (
            <div className="space-y-2">
              {comparison.semantic.map((change, index) => (
                <SemanticChangeRow key={`${change.path}:${index}`} change={change} />
              ))}
            </div>
          ) : (
            <div className="text-sm text-[var(--muted)]">No manifest-level changes.</div>
          )}
        </Card.Body>
      </Card>

      <Card className="overflow-hidden p-0 gap-0">
        <Card.Header>
          <div className="min-w-0">
            <Card.Title>File patches</Card.Title>
            <Card.Subtitle className="truncate">{comparison.skillPath}</Card.Subtitle>
          </div>
          <Pill tone={comparison.files.length ? "default" : "success"}>
            {comparison.files.length} files
          </Pill>
        </Card.Header>
        <Card.Body>
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
        </Card.Body>
      </Card>
    </div>
  );
}
