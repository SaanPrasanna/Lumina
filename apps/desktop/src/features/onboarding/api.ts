import { call } from "@/lib/ipc";

/** Mirrors `BootstrapStatus` in src-tauri/src/commands/app.rs. */
export interface BootstrapStatus {
  configured: boolean;
  dataRoot: string | null;
  defaultDataRoot: string;
  language: string;
}

export const bootstrapStatus = () => call<BootstrapStatus>("bootstrap_status");

export const bootstrapComplete = (dataRoot: string, language: string) =>
  call<BootstrapStatus>("bootstrap_complete", { dataRoot, language });

export const setLanguage = (language: string) => call<void>("set_language", { language });

export const appVersion = () => call<string>("app_version");

export const BOOTSTRAP_QUERY_KEY = ["bootstrap"] as const;
