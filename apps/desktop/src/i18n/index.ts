import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en/common.json";
import si from "./locales/si/common.json";

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

export const resources = {
  en: { common: en },
  si: { common: si },
} as const;

function applyDocumentLanguage(lng: string) {
  document.documentElement.lang = lng;
  document.documentElement.dir = RTL_LANGUAGES.includes(lng) ? "rtl" : "ltr";
}

// Registered before init so the initial language is applied too.
i18n.on("languageChanged", applyDocumentLanguage);

void i18n.use(initReactI18next).init({
  resources,
  lng: "en",
  fallbackLng: "en",
  supportedLngs: SUPPORTED_LANGUAGES,
  defaultNS: "common",
  interpolation: { escapeValue: false },
});

export default i18n;
