import { invoke } from "@tauri-apps/api/core";

/** Thin typed wrapper around Tauri `invoke`; all feature `api.ts` files go through this. */
export function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args);
}
