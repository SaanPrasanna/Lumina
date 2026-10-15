import { useQueryClient } from "@tanstack/react-query";
import { Languages } from "lucide-react";
import { useTranslation } from "react-i18next";
import { BOOTSTRAP_QUERY_KEY, setLanguage } from "@/features/onboarding/api";
import { changeLanguage, isLanguage, LANGUAGE_NAMES, SUPPORTED_LANGUAGES } from "@/i18n";

/** Switches the UI language and persists it in the backend bootstrap file. */
export function LanguageSelect() {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();

  const change = async (value: string) => {
    if (!isLanguage(value)) return;
    await changeLanguage(value);
    try {
      await setLanguage(value);
      await queryClient.invalidateQueries({ queryKey: BOOTSTRAP_QUERY_KEY });
    } catch (err) {
      console.error("failed to persist language", err);
    }
  };

  return (
    <label className="flex items-center gap-2 text-xs text-muted">
      <Languages className="size-4 shrink-0" aria-hidden />
      <span className="sr-only">{t("language.label")}</span>
      <select
        className="min-w-0 flex-1 rounded-md border border-line bg-surface px-2 py-1 text-sm text-fg"
        value={i18n.resolvedLanguage}
        onChange={(e) => void change(e.target.value)}
      >
        {SUPPORTED_LANGUAGES.map((lng) => (
          <option key={lng} value={lng}>
            {LANGUAGE_NAMES[lng]}
          </option>
        ))}
      </select>
    </label>
  );
}
