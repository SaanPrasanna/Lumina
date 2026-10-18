import { useTranslation } from "react-i18next";
import { NavLink } from "react-router";
import { NAV_ITEMS } from "@/app/nav";
import { LanguageSelect } from "@/components/LanguageSelect";
import { ThemeSelect } from "@/components/ThemeSelect";
import { ProfileSwitcher } from "@/features/profiles/ProfileSwitcher";

export function Sidebar() {
  const { t } = useTranslation();

  return (
    // No logo or name here: the window title already says "Lumina".
    <aside className="flex w-56 shrink-0 flex-col border-e border-line bg-sidebar pt-3">

      <ProfileSwitcher />

      <nav className="flex-1 overflow-y-auto px-2" aria-label={t("app.title")}>
        <ul className="space-y-0.5">
          {NAV_ITEMS.map(({ path, labelKey, icon: Icon }) => (
            <li key={path}>
              <NavLink
                to={path}
                end={path === "/"}
                className={({ isActive }) =>
                  `flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm outline-none transition-colors duration-150 ease-out focus-visible:ring-2 focus-visible:ring-accent ${
                    isActive
                      ? "bg-accent-soft font-medium text-accent-ink"
                      : "text-fg-soft hover:bg-hover"
                  }`
                }
              >
                <Icon className="size-4 shrink-0" aria-hidden />
                <span className="truncate">{t(labelKey)}</span>
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>

      <div className="space-y-2 border-t border-line p-3">
        <LanguageSelect />
        <ThemeSelect />
      </div>
    </aside>
  );
}
