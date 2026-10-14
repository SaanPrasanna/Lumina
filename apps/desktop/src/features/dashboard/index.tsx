import { useQuery } from "@tanstack/react-query";
import { FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { PageHeader } from "@/components/layout/PageHeader";
import { BOOTSTRAP_QUERY_KEY, bootstrapStatus } from "@/features/onboarding/api";

export function DashboardPage() {
  const { t } = useTranslation();
  const { data } = useQuery({ queryKey: BOOTSTRAP_QUERY_KEY, queryFn: bootstrapStatus });

  return (
    <>
      <PageHeader title={t("nav.dashboard")} />
      <div className="space-y-4 px-8">
        <p className="text-sm text-muted">{t("dashboard.empty")}</p>
        {data?.dataRoot && (
          <div className="flex items-center gap-3 rounded-lg border border-line p-4">
            <FolderOpen className="size-5 shrink-0 text-accent" aria-hidden />
            <div className="min-w-0">
              <div className="text-xs text-muted">{t("dashboard.dataRoot")}</div>
              <div className="truncate font-mono text-sm" dir="ltr">
                {data.dataRoot}
              </div>
            </div>
          </div>
        )}
      </div>
    </>
  );
}
