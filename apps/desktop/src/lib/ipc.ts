import { invoke } from "@tauri-apps/api/core";

/** Mirrors `CommandError` in src-tauri/src/error.rs. */
export interface CommandError {
  code: string;
  message: string;
}

export function isCommandError(value: unknown): value is CommandError {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

/** Thin typed wrapper around Tauri `invoke`; all feature `api.ts` files go through this. */
export function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args);
}
