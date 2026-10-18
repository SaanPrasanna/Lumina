import type { CommandError } from "@/lib/bindings";

// Commands and their types are generated from Rust: see `commands` in
// src/lib/bindings.ts (each feature's api.ts wraps the ones it uses).
export type { CommandError };

export function isCommandError(value: unknown): value is CommandError {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

/** What is missing, from a `not_installed` error. */
export interface Missing {
  /** Quick Add package id. */
  package: string;
  name: string;
  version: string;
}

export function missingOf(error: unknown): Missing | null {
  if (!isCommandError(error) || error.code !== "not_installed" || typeof error.details !== "object" || error.details === null) return null;
  const { package: pkg, name, version } = error.details as Record<string, unknown>;
  if (typeof pkg !== "string" || typeof name !== "string") return null;
  return { package: pkg, name, version: typeof version === "string" ? version : "" };
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
