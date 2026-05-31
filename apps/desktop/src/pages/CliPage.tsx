import { Button } from "@heroui/react";
import { PackageOpen, Terminal } from "lucide-react";
import { useLocale } from "../hooks/useLocale";
import type { DiagnosticsExport } from "../lib/teamai";
import { Card } from "../widgets/Card";
import { MetricTile } from "../widgets/MetricTile";
import { Pill } from "../widgets/Pill";

export function CliPage({
  diagnostics,
  diagnosticsError,
  diagnosticsPending,
  logsError,
  logsPending,
  onExportDiagnostics,
  onOpenLogs,
}: {
  diagnostics: DiagnosticsExport | null;
  diagnosticsError: string | null;
  diagnosticsPending: boolean;
  logsError: string | null;
  logsPending: boolean;
  onExportDiagnostics: () => void;
  onOpenLogs: () => void;
}) {
  const { t } = useLocale();
  return (
    <section className="scroll-area min-h-0 flex-1 px-6 py-6">
      <div className="mx-auto flex max-w-4xl flex-col gap-5">
        <Card className="p-5">
          <div className="mb-3 flex items-start justify-between gap-3">
            <div>
              <Card.Title>{t("cli.rustCli")}</Card.Title>
              <Card.Subtitle>{t("cli.rustCliDesc")}</Card.Subtitle>
            </div>
            <Pill tone="success">native</Pill>
          </div>
          <pre className="code-panel compact">teamai --help{"\n"}teamai sync{"\n"}teamai status --target claude-code --target cursor --target codex</pre>
          <p className="mt-3 text-[12px] text-[var(--muted)]">
            {t("cli.cliHint")}
          </p>
        </Card>

        <Card className="p-5">
          <div className="mb-4 grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
            <div className="min-w-0">
              <Card.Title>{t("cli.diagnostics")}</Card.Title>
              <Card.Subtitle className="truncate">
                {diagnostics?.outputDir ?? "~/.team-ai-hub/tmp/diagnostics"}
              </Card.Subtitle>
            </div>
            <div className="flex flex-wrap justify-end gap-2">
              <Button variant="secondary" onPress={onExportDiagnostics} isPending={diagnosticsPending}>
                <PackageOpen size={14} />
                {t("cli.export")}
              </Button>
              <Button variant="outline" onPress={onOpenLogs} isPending={logsPending}>
                <Terminal size={14} />
                {t("cli.openLogs")}
              </Button>
            </div>
          </div>

          {diagnosticsError ? (
            <div className="mb-3 rounded-md border border-[#f0c6b8] bg-[#fcefe7]/60 px-3 py-2 text-xs text-[var(--status-danger)]">
              {diagnosticsError}
            </div>
          ) : null}
          {logsError ? (
            <div className="mb-3 rounded-md border border-[#f0c6b8] bg-[#fcefe7]/60 px-3 py-2 text-xs text-[var(--status-danger)]">
              {logsError}
            </div>
          ) : null}

          {diagnostics ? (
            <div className="grid gap-3 md:grid-cols-3">
              <MetricTile
                label={t("cli.subscriptions")}
                value={diagnostics.subscriptions}
                tone={diagnostics.subscriptions ? "success" : "default"}
              />
              <MetricTile
                label={t("cli.workspaces")}
                value={diagnostics.workspaces}
                tone={diagnostics.workspaces ? "success" : "default"}
              />
              <MetricTile
                label={t("cli.logs")}
                value={diagnostics.logs.length}
                tone={diagnostics.logs.length ? "success" : "default"}
              />
            </div>
          ) : null}

          {diagnostics?.notes.length ? (
            <div className="mt-3 flex flex-wrap gap-1.5">
              {diagnostics.notes.map((note) => (
                <Pill key={note}>{note}</Pill>
              ))}
            </div>
          ) : null}
        </Card>
      </div>
    </section>
  );
}
