import { useTranslation } from "react-i18next";
import type { ServiceStatus } from "@/types";

const DOT: Record<ServiceStatus, string> = {
  running: "bg-ok",
  starting: "bg-warn animate-pulse",
  stopping: "bg-warn animate-pulse",
  stopped: "bg-muted",
  failed: "bg-danger",
};

export function StatusBadge({ status }: { status: ServiceStatus }) {
  const { t } = useTranslation();
  return (
    <span className="inline-flex items-center gap-1.5 text-sm text-fg-soft">
      <span className={`size-2.5 rounded-full ${DOT[status]}`} aria-hidden />
      {t(`status.${status}`)}
    </span>
  );
}
