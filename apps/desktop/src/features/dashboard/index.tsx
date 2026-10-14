import { useTranslation } from "react-i18next";

export function DashboardPage() {
  const { t } = useTranslation();
  return <h1 className="p-6 text-xl font-semibold">{t("app.title")}</h1>;
}
