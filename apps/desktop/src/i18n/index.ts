import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en/common.json";

/** RTL languages flip `dir` on <html>. */
export const RTL_LANGUAGES = ["ur"];
export const SUPPORTED_LANGUAGES = ["en", "cs", "ur", "tr", "hi", "th", "si"] as const;

i18n.use(initReactI18next).init({
  resources: { en: { common: en } },
  lng: "en",
  fallbackLng: "en",
  defaultNS: "common",
  interpolation: { escapeValue: false },
});

i18n.on("languageChanged", (lng) => {
  document.documentElement.lang = lng;
  document.documentElement.dir = RTL_LANGUAGES.includes(lng) ? "rtl" : "ltr";
});

export default i18n;
