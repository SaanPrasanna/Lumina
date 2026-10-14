import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { ConfigWarnings } from "@/components/ConfigWarnings";
import { PageHeader } from "@/components/layout/PageHeader";
import { PhpCard } from "@/features/services/PhpCard";
import { SETTINGS_KEY, settingsGet } from "@/features/settings/api";

export function ServicesPage() {
  const { t } = useTranslation();
  const settings = useQuery({ queryKey: SETTINGS_KEY, queryFn: settingsGet });

  return (
    <>
      <PageHeader title={t("nav.services")} />
      <div className="space-y-4 px-8 pb-8">
        <ConfigWarnings warnings={settings.data?.warnings ?? []} />
        <PhpCard />
      </div>
    </>
  );
}
