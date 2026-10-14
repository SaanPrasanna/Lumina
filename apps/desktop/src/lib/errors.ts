import type { TFunction } from "i18next";
import { isCommandError } from "@/lib/ipc";

/** Translated headline + English detail for a failed backend command. */
export function describeError(t: TFunction, error: unknown): { title: string; detail?: string } {
  if (isCommandError(error)) {
    return {
      title: t(`errors.${error.code}`, { defaultValue: t("errors.unknown") }),
      detail: error.message,
    };
  }
  return { title: t("errors.unknown"), detail: error ? String(error) : undefined };
}
