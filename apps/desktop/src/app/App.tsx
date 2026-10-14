import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AppRouter } from "@/app/router";
import { useApplyTheme } from "@/app/theme";
import { FirstRunPage } from "@/features/onboarding/FirstRunPage";
import { BOOTSTRAP_QUERY_KEY, bootstrapStatus } from "@/features/onboarding/api";
import { isLanguage } from "@/i18n";
import { useBackendEvents } from "@/lib/events";

/** Shows the first-run wizard until a data root is chosen, then the main app. */
export function App() {
  useApplyTheme();
  useBackendEvents();
  const { i18n } = useTranslation();
  const status = useQuery({ queryKey: BOOTSTRAP_QUERY_KEY, queryFn: bootstrapStatus });

  // Apply the saved language *before* rendering the UI. Switching after the
  // first paint left components that were mid-mount (React StrictMode mounts
  // twice) showing the old language.
  const savedLanguage = status.data?.language;
  const [appliedLanguage, setAppliedLanguage] = useState<string | null>(null);
  useEffect(() => {
    if (!savedLanguage) return;
    const target = isLanguage(savedLanguage) ? savedLanguage : "en";
    if (i18n.language === target) {
      setAppliedLanguage(target);
      return;
    }
    void i18n.changeLanguage(target).then(() => setAppliedLanguage(target));
  }, [savedLanguage, i18n]);

  if (status.isPending || (status.isSuccess && appliedLanguage === null)) {
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
