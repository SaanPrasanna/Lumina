// IPC wrappers for HTTPS (see src-tauri/src/commands/ssl.rs).
import { commands } from "@/lib/bindings";

export type { CaStatus, SiteCert, SslOverview, TrustOutcome } from "@/lib/bindings";

/** A date such as "15 Nov 2027" from Unix seconds, in the UI language. */
export function formatDate(unix: number, language: string): string {
  return new Date(unix * 1000).toLocaleDateString(language, { year: "numeric", month: "short", day: "numeric" });
}

export const SSL_KEY = ["sites", "ssl"] as const;

export const sslStatus = () => commands.sslStatus();
/** "declined" when the user answered "No" to the OS prompt; null when nothing was asked. */
export const sslSetEnabled = (enabled: boolean) => commands.sslSetEnabled(enabled);
export const sslSetForceHttps = (force: boolean) => commands.sslSetForceHttps(force);
export const sslTrust = () => commands.sslTrust();
export const sslRegenerate = () => commands.sslRegenerate();
export const sslRemoveCa = () => commands.sslRemoveCa();
