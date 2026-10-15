import { describe, expect, it } from "vitest";
import { loadMessages, SUPPORTED_LANGUAGES } from "@/i18n";

/** The app's code (tests excluded), as text. */
const SOURCES = import.meta.glob(["/src/**/*.{ts,tsx}", "!/src/**/*.test.{ts,tsx}"], {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

/** `key` exists, or its plural forms (`key_one`, `key_other`) do. */
function has(messages: object, key: string): boolean {
  return lookup(messages, key) || lookup(messages, `${key}_other`);
}

function lookup(messages: object, key: string): boolean {
  let node: unknown = messages;
  for (const part of key.split(".")) {
    if (typeof node !== "object" || node === null || !(part in node)) return false;
    node = (node as Record<string, unknown>)[part];
  }
  return typeof node === "string" || (typeof node === "object" && node !== null);
}

/** Literal keys passed to t("…") or i18nKey in the app's code. */
function usedKeys(): string[] {
  const keys = new Set<string>();
  for (const text of Object.values(SOURCES)) {
    for (const m of text.matchAll(/\bt\(\s*["']([a-zA-Z][\w-]*(?:\.[\w-]+)+)["']/g)) keys.add(m[1]);
    // Keys kept in tables, e.g. ["general", "settings.general.title"].
    for (const m of text.matchAll(/["']((?:settings|logs|dashboard|nav|tunnel|sites|errors)\.[\w-]+(?:\.[\w-]+)*)["']/g)) {
      keys.add(m[1]);
    }
  }
  return [...keys].sort();
}

describe("translation keys", () => {
  const keys = usedKeys();

  it("finds the keys used in code", () => {
    expect(keys.length).toBeGreaterThan(300);
  });

  it.each(SUPPORTED_LANGUAGES)("every key used in code exists in %s", async (lng) => {
    const messages = await loadMessages(lng);
    expect(keys.filter((k) => !has(messages, k))).toEqual([]);
  });
});
