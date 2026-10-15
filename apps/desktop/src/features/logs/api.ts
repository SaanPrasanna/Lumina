// IPC wrappers for the `logs` feature (see src-tauri/src/commands/logs.rs).
import { call } from "@/lib/ipc";

/** Mirrors `LogSource`. */
export interface LogSource {
  /** `nginx_error.log`, `quickapp/blog.log` or `app`. */
  id: string;
  group: "app" | "service" | "quickapp";
  path: string;
  size: number;
  /** Unix seconds. */
  modified: number;
}

/** Mirrors `lumina_process::logs::Chunk`. */
export interface Chunk {
  text: string;
  offset: number;
  size: number;
  /** The file was rotated: drop what was shown. */
  reset: boolean;
  /** Older text was left out. */
  skipped: boolean;
}

export const LOGS_KEY = ["logs"] as const;

export const logsList = () => call<LogSource[]>("logs_list");
/** `offset` null: start with the end of the log. */
export const logsRead = (id: string, offset: number | null) => call<Chunk>("logs_read", { id, offset });
export const logsOpen = (id: string) => call<void>("logs_open", { id });
export const logsOpenFolder = () => call<void>("logs_open_folder");

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
