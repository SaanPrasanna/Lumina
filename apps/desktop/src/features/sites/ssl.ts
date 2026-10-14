// IPC wrappers for HTTPS (see src-tauri/src/commands/ssl.rs).
import { call } from "@/lib/ipc";

/** Mirrors `CaStatus` (lumina_ssl::CaInfo + trust state). */
export interface CaStatus {
  subject: string;
  /** Unix seconds. */
  notAfter: number;
  thumbprint: string;
  permitted: string[];
  certPath: string;
  trusted: boolean;
  coversTld: boolean;
}

export interface SiteCert {
  domain: string;
  /** Unix seconds. */
  notAfter: number;
}

/** Mirrors `SslOverview`. */
export interface SslOverview {
  enabled: boolean;
  forceHttps: boolean;
  httpsPort: number;
  tld: string;
  ca: CaStatus | null;
  sites: SiteCert[];
  error: string | null;
  sslDir: string;
}

/** "done", or "declined" when the user answered "No" to the OS prompt. */
export type TrustOutcome = "done" | "declined";

/** A date such as "15 Nov 2027" from Unix seconds, in the UI language. */
export function formatDate(unix: number, language: string): string {
  return new Date(unix * 1000).toLocaleDateString(language, { year: "numeric", month: "short", day: "numeric" });
}

export const SSL_KEY = ["sites", "ssl"] as const;

export const sslStatus = () => call<SslOverview>("ssl_status");
export const sslSetEnabled = (enabled: boolean) => call<TrustOutcome | null>("ssl_set_enabled", { enabled });
export const sslSetForceHttps = (force: boolean) => call<void>("ssl_set_force_https", { force });
export const sslTrust = () => call<TrustOutcome>("ssl_trust");
export const sslRegenerate = () => call<TrustOutcome>("ssl_regenerate");
export const sslRemoveCa = () => call<TrustOutcome>("ssl_remove_ca");
