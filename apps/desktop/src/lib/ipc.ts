import { invoke } from "@tauri-apps/api/core";

/** Mirrors `CommandError` in src-tauri/src/error.rs. */
export interface CommandError {
  code: string;
  message: string;
  /** Structured data for some codes, e.g. `{ port, owner }` for `port_in_use`. */
  details?: Record<string, unknown>;
}

export function isCommandError(value: unknown): value is CommandError {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

/** Thin typed wrapper around Tauri `invoke`; all feature `api.ts` files go through this. */
export function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args);
}

/** A port another program holds, from a `port_in_use` error. */
export interface PortConflict {
  port: number;
  owner: string | null;
}

/** The conflict described by `error` if it is `port_in_use` for `port`. */
export function portConflictOf(error: unknown, port: number | undefined): PortConflict | null {
  if (!isCommandError(error) || error.code !== "port_in_use" || !error.details) return null;
  const { port: busy, owner } = error.details;
  if (typeof busy !== "number" || busy !== port) return null;
  return { port: busy, owner: typeof owner === "string" ? owner : null };
}
