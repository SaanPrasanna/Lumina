import { describe, expect, it } from "vitest";
import i18n, { resources, SUPPORTED_LANGUAGES } from "@/i18n";

/** Flattens nested locale JSON into dotted keys. */
function keys(obj: object, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([k, v]) =>
    typeof v === "object" && v !== null ? keys(v, `${prefix}${k}.`) : [`${prefix}${k}`],
  );
}

describe("i18n", () => {
  const english = keys(resources.en.common).sort();

  it.each(SUPPORTED_LANGUAGES)("%s has exactly the English keys", (lng) => {
    expect(keys(resources[lng].common).sort()).toEqual(english);
  });

  it.each(SUPPORTED_LANGUAGES)("%s has no empty strings", (lng) => {
    for (const key of keys(resources[lng].common)) {
      expect(i18n.getFixedT(lng)(key), `${lng}:${key}`).not.toBe("");
    }
  });

  it("sets the document language and keeps Sinhala left-to-right", async () => {
    await i18n.changeLanguage("si");
    expect(document.documentElement.lang).toBe("si");
    expect(document.documentElement.dir).toBe("ltr");
    expect(i18n.t("nav.settings")).toBe("සැකසීම්");

    await i18n.changeLanguage("en");
    expect(document.documentElement.lang).toBe("en");
  });
});
