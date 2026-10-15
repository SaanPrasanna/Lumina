import { getCurrentWindow } from "@tauri-apps/api/window";
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

/** The page background (--color-surface) as RGB, per theme. */
const SURFACE: Record<"light" | "dark", [number, number, number]> = {
  light: [255, 255, 255],
  dark: [11, 17, 32],
};

/**
 * The native window paints this colour before (and while resizing) the web
 * page, so a dark theme never shows white edges. No-op outside Tauri.
 */
function setWindowBackground(dark: boolean) {
  if (!("__TAURI_INTERNALS__" in window)) return;
  getCurrentWindow()
    .setBackgroundColor(SURFACE[dark ? "dark" : "light"])
    .catch(() => {});
}

/** Keeps the `dark` class on <html> in sync with the chosen theme and the OS setting. */
export function useApplyTheme() {
  const theme = useThemeStore((s) => s.theme);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = theme === "dark" || (theme === "system" && media.matches);
      document.documentElement.classList.toggle("dark", dark);
      setWindowBackground(dark);
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
}
