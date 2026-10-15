// IPC wrappers for the `packages` feature (see src-tauri/src/commands/packages.rs).
import { commands } from "@/lib/bindings";

export type {
  AvailableVersion,
  InstalledVersion,
  Kind as PackageKind,
  PackageJob as Job,
  PackagesOverview,
  PackageView,
  Stage,
} from "@/lib/bindings";

/** Payload of the `package-progress` event. */
export interface ProgressEvent {
  id: string;
  version: string;
  stage: import("@/lib/bindings").Stage;
  received: number;
  total: number | null;
}

export const PACKAGES_KEY = ["packages"] as const;

export const packagesList = () => commands.packagesList();
export const packageInstall = (id: string, version: string) => commands.packageInstall(id, version);
export const packageCancel = (id: string, version: string) => commands.packageCancel(id, version);
export const packageDismiss = (id: string, version: string) => commands.packageDismiss(id, version);
export const packageUninstall = (id: string, version: string) => commands.packageUninstall(id, version);
/** `version` "" = newest installed (phpMyAdmin: newest that runs on the active PHP). */
export const packageSetActive = (id: string, version: string) => commands.packageSetActive(id, version);
export const dbUiOpen = () => commands.dbUiOpen();

/** "33.5 MB" style size. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}
