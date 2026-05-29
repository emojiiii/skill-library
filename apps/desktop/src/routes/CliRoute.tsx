import { useMutation } from "@tanstack/react-query";
import { exportDiagnostics, openLogsFolder } from "../lib/teamai";
import { CliPage } from "../pages/CliPage";
import { formatError } from "../utils/format";

export function CliRoute() {
  const diagnostics = useMutation({ mutationFn: exportDiagnostics });
  const logsFolder = useMutation({ mutationFn: openLogsFolder });

  return (
    <CliPage
      diagnostics={diagnostics.data ?? null}
      diagnosticsError={diagnostics.error ? formatError(diagnostics.error) : null}
      diagnosticsPending={diagnostics.isPending}
      logsError={logsFolder.error ? formatError(logsFolder.error) : null}
      logsPending={logsFolder.isPending}
      onExportDiagnostics={() => diagnostics.mutate()}
      onOpenLogs={() => logsFolder.mutate()}
    />
  );
}
