import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { AppRouter } from "@/app/router";
import { useApplyTheme } from "@/app/theme";
import { FirstRunPage } from "@/features/onboarding/FirstRunPage";
import { BOOTSTRAP_QUERY_KEY, bootstrapStatus } from "@/features/onboarding/api";
import { isLanguage } from "@/i18n";

/** Shows the first-run wizard until a data root is chosen, then the main app. */
export function App() {
  useApplyTheme();
  const { i18n } = useTranslation();
  const status = useQuery({ queryKey: BOOTSTRAP_QUERY_KEY, queryFn: bootstrapStatus });

  // Apply the persisted language once the backend reports it.
  const savedLanguage = status.data?.language;
  useEffect(() => {
    if (savedLanguage && isLanguage(savedLanguage) && savedLanguage !== i18n.language) {
      void i18n.changeLanguage(savedLanguage);
    }
  }, [savedLanguage, i18n]);

  if (status.isPending) {
    return <div className="h-full bg-surface" aria-busy="true" />;
  }
  if (status.isError) {
    // Outside Tauri (e.g. `pnpm dev:web` in a browser) there is no backend.
    return (
      <div className="flex h-full items-center justify-center bg-surface p-6 text-sm text-muted">
        {String(status.error)}
      </div>
    );
  }
  return status.data.configured ? <AppRouter /> : <FirstRunPage status={status.data} />;
}
