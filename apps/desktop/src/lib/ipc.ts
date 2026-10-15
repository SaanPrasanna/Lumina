import type { CommandError } from "@/lib/bindings";

// Commands and their types are generated from Rust: see `commands` in
// src/lib/bindings.ts (each feature's api.ts wraps the ones it uses).
export type { CommandError };

export function isCommandError(value: unknown): value is CommandError {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

/** A port another program holds, from a `port_in_use` error. */
export interface PortConflict {
  port: number;
  owner: string | null;
}

/** The conflict described by `error` if it is `port_in_use` for `port`. */
export function portConflictOf(error: unknown, port: number | undefined): PortConflict | null {
  if (!isCommandError(error) || error.code !== "port_in_use" || typeof error.details !== "object" || error.details === null) return null;
  const { port: busy, owner } = error.details as Record<string, unknown>;
  if (typeof busy !== "number" || busy !== port) return null;
  return { port: busy, owner: typeof owner === "string" ? owner : null };
}
