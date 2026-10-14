import { useTranslation } from "react-i18next";
import { PageHeader } from "@/components/layout/PageHeader";

/** Placeholder for feature pages that later phases implement. */
export function ComingSoonPage({ titleKey }: { titleKey: string }) {
  const { t } = useTranslation();
  return (
    <>
      <PageHeader title={t(titleKey)} />
      <p className="px-8 text-sm text-muted">{t("page.comingSoon")}</p>
    </>
  );
}
