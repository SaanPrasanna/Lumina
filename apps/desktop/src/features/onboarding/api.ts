// IPC wrappers for the first-run wizard and app info (see src-tauri/src/commands/app.rs).
import { commands } from "@/lib/bindings";

export type { BootstrapStatus } from "@/lib/bindings";

export const bootstrapStatus = () => commands.bootstrapStatus();

export const bootstrapComplete = (dataRoot: string, language: string) => commands.bootstrapComplete(dataRoot, language);

export const setLanguage = (language: string) => commands.setLanguage(language);

export const appVersion = () => commands.appVersion();

export const BOOTSTRAP_QUERY_KEY = ["bootstrap"] as const;
