import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en/common.json";

/** Must match `SUPPORTED_LANGUAGES` in crates/lumina-core/src/constants.rs. */
export const SUPPORTED_LANGUAGES = ["en", "si"] as const;
export type Language = (typeof SUPPORTED_LANGUAGES)[number];

/** Endonyms: each language is listed in its own script. */
export const LANGUAGE_NAMES: Record<Language, string> = {
  en: "English",
  si: "සිංහල",
};

/**
 * Right-to-left languages flip `dir` on <html>; layouts use logical CSS properties
 * so they mirror automatically. Empty until an RTL language (e.g. Urdu) is added.
 */
export const RTL_LANGUAGES: readonly string[] = [];

export function isLanguage(value: string): value is Language {
  return (SUPPORTED_LANGUAGES as readonly string[]).includes(value);
}

type Messages = typeof en;

/**
 * English is the source and the fallback, so it is always bundled; every other
 * language is a separate chunk, loaded the first time it is chosen.
 */
const LOADERS: Record<Language, () => Promise<{ default: Messages }>> = {
  en: () => Promise.resolve({ default: en }),
  si: () => import("./locales/si/common.json"),
};

/** The messages of `lng` (loads them if needed). */
export async function loadMessages(lng: Language): Promise<Messages> {
  return (await LOADERS[lng]()).default;
}

/** Loads `lng` if needed, then switches to it. */
export async function changeLanguage(lng: Language): Promise<void> {
  if (!i18n.hasResourceBundle(lng, "common")) {
    i18n.addResourceBundle(lng, "common", await loadMessages(lng), true, true);
  }
  await i18n.changeLanguage(lng);
}

function applyDocumentLanguage(lng: string) {
  document.documentElement.lang = lng;
  document.documentElement.dir = RTL_LANGUAGES.includes(lng) ? "rtl" : "ltr";
}

// Registered before init so the initial language is applied too.
i18n.on("languageChanged", applyDocumentLanguage);

void i18n.use(initReactI18next).init({
  resources: { en: { common: en } },
  lng: "en",
  fallbackLng: "en",
  supportedLngs: SUPPORTED_LANGUAGES,
  defaultNS: "common",
  interpolation: { escapeValue: false },
  // Languages are added at runtime by `changeLanguage`.
  partialBundledLanguages: true,
});

export default i18n;
