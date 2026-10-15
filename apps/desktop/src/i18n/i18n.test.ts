import { describe, expect, it } from "vitest";
import i18n, { changeLanguage, loadMessages, SUPPORTED_LANGUAGES } from "@/i18n";

/** Flattens nested locale JSON into dotted keys. */
function keys(obj: object, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([k, v]) =>
    typeof v === "object" && v !== null ? keys(v, `${prefix}${k}.`) : [`${prefix}${k}`],
  );
}

describe("i18n", () => {
  it.each(SUPPORTED_LANGUAGES)("%s has exactly the English keys", async (lng) => {
    const english = keys(await loadMessages("en")).sort();
    expect(keys(await loadMessages(lng)).sort()).toEqual(english);
  });

  it.each(SUPPORTED_LANGUAGES)("%s has no empty strings", async (lng) => {
    const messages = await loadMessages(lng);
    for (const key of keys(messages)) {
      const value = key.split(".").reduce<unknown>((node, k) => (node as Record<string, unknown>)[k], messages);
      expect(value, `${lng}:${key}`).not.toBe("");
    }
  });

  it("loads Sinhala only when it is chosen", async () => {
    expect(i18n.hasResourceBundle("en", "common")).toBe(true);
    expect(i18n.hasResourceBundle("si", "common")).toBe(false);
    await changeLanguage("si");
    expect(i18n.hasResourceBundle("si", "common")).toBe(true);
    await changeLanguage("en");
  });

  it("sets the document language and keeps Sinhala left-to-right", async () => {
    await changeLanguage("si");
    expect(document.documentElement.lang).toBe("si");
    expect(document.documentElement.dir).toBe("ltr");
    expect(i18n.t("nav.settings")).toBe("සැකසීම්");

    await i18n.changeLanguage("en");
    expect(document.documentElement.lang).toBe("en");
  });
});
