// IPC wrappers for the `logs` feature (see src-tauri/src/commands/logs.rs).
import { commands } from "@/lib/bindings";

export type { Chunk, LogSource } from "@/lib/bindings";

export const LOGS_KEY = ["logs"] as const;

export const logsList = () => commands.logsList();
/** `offset` null: start with the end of the log. */
export const logsRead = (id: string, offset: number | null) => commands.logsRead(id, offset);
export const logsOpen = (id: string) => commands.logsOpen(id);
export const logsOpenFolder = () => commands.logsOpenFolder();

export type Level = "error" | "warn" | "info";

/** A line's severity, from the words servers use (nginx `[error]`, ngrok `"lvl":"eror"`, PHP `Fatal error`). */
export function levelOf(line: string): Level {
  if (/\b(error|eror|crit|fatal|alert|emerg|panic|failed)\b/i.test(line)) return "error";
  if (/\b(warn|warning)\b/i.test(line)) return "warn";
  return "info";
}

/**
 * Appends `text` to `lines`, completing a line cut off by the previous read
 * (`pending`). Returns the new pending fragment.
 */
export function appendText(lines: string[], pending: string, text: string): string {
  const parts = (pending + text).split(/\r?\n/);
  const rest = parts.pop() ?? "";
  lines.push(...parts);
  return rest;
}
