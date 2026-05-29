import { Button, Tooltip } from "@heroui/react";
import { RefreshCw } from "lucide-react";
import { useLocale } from "../hooks/useLocale";
import { type AppPage, pageCopyKeys } from "../utils/navigation";

export function Topbar({
  page,
  onRefresh,
  refreshing,
  primaryAction,
}: {
  page: AppPage;
  onRefresh: () => void;
  refreshing: boolean;
  primaryAction?: React.ReactNode;
}) {
  const { t } = useLocale();
  const keys = pageCopyKeys[page];
  return (
    <header className="topbar">
      <div className="min-w-0">
        <div className="topbar__title">{t(keys.titleKey)}</div>
        <div className="topbar__breadcrumb truncate">{t(keys.subtitleKey)}</div>
      </div>
      <div className="flex items-center gap-1.5">
        {primaryAction}
        <Tooltip delay={0}>
          <Button isIconOnly size="sm" variant="tertiary" onPress={onRefresh} isPending={refreshing}>
            <RefreshCw size={14} />
          </Button>
          <Tooltip.Content>{t("topbar.refresh")}</Tooltip.Content>
        </Tooltip>
      </div>
    </header>
  );
}
