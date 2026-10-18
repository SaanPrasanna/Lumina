import type { TFunction } from "i18next";
import { isCommandError, missingOf } from "@/lib/ipc";

/** Translated headline + English detail for a failed backend command. */
export function describeError(t: TFunction, error: unknown): { title: string; detail?: string } {
  const missing = missingOf(error);
  if (missing) {
    // The headline says it all; no English detail underneath.
    return { title: t(missing.version ? "errors.not_installed_version" : "errors.not_installed", { name: missing.name, version: missing.version }) };
  }
  if (isCommandError(error)) {
    return {
      title: t(`errors.${error.code}`, { defaultValue: t("errors.unknown") }),
      detail: error.message,
    };
  }
  return { title: t("errors.unknown"), detail: error ? String(error) : undefined };
}
