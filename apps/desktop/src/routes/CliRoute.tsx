import { toast } from "@heroui/react";
import { useMutation } from "@tanstack/react-query";
import { exportDiagnostics, openLogsFolder } from "../lib/skill-library";
import { CliPage } from "../pages/CliPage";
import { useLocale } from "../hooks/useLocale";
import { formatError } from "../utils/format";

export function CliRoute() {
  const { t } = useLocale();
  const diagnostics = useMutation({
    mutationFn: exportDiagnostics,
    onSuccess: (report) =>
      toast.success(t("cli.exportSuccess"), {
        description: t("cli.exportSuccessDesc").replace("{path}", report.archivePath),
      }),
    onError: (err) => toast.danger(formatError(err)),
  });
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
