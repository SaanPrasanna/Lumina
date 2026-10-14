import { Monitor, Moon, Sun } from "lucide-react";
import { useTranslation } from "react-i18next";
import { type Theme, useThemeStore } from "@/app/theme";

const OPTIONS: { value: Theme; icon: typeof Sun }[] = [
  { value: "system", icon: Monitor },
  { value: "light", icon: Sun },
  { value: "dark", icon: Moon },
];

/** Segmented System / Light / Dark control. */
export function ThemeSelect() {
  const { t } = useTranslation();
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);

  return (
    <div role="radiogroup" aria-label={t("theme.label")} className="flex rounded-md border border-line p-0.5">
      {OPTIONS.map(({ value, icon: Icon }) => (
        <button
          key={value}
          type="button"
          role="radio"
          aria-checked={theme === value}
          title={t(`theme.${value}`)}
          onClick={() => setTheme(value)}
          className={`flex flex-1 justify-center rounded py-1 outline-none focus-visible:ring-2 focus-visible:ring-accent ${
            theme === value ? "bg-accent-soft text-accent" : "text-muted hover:bg-hover"
          }`}
        >
          <Icon className="size-4" aria-hidden />
          <span className="sr-only">{t(`theme.${value}`)}</span>
        </button>
      ))}
    </div>
  );
}
