import { useEffect } from "react";
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export type Theme = "system" | "light" | "dark";

interface ThemeState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

/** Theme is a per-device UI preference, so browser storage is the right home for it. */
export const useThemeStore = create<ThemeState>()(
  persist(
    (set) => ({
      theme: "system",
      setTheme: (theme) => set({ theme }),
    }),
    { name: "lumina.theme", storage: createJSONStorage(() => localStorage) },
  ),
);

/** Keeps the `dark` class on <html> in sync with the chosen theme and the OS setting. */
export function useApplyTheme() {
  const theme = useThemeStore((s) => s.theme);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = theme === "dark" || (theme === "system" && media.matches);
      document.documentElement.classList.toggle("dark", dark);
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
}
