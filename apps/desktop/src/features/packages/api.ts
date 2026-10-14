// IPC wrappers for the `packages` feature (see src-tauri/src/commands/packages.rs).
import { call } from "@/lib/ipc";

export type PackageKind = "service" | "runtime" | "tool" | "webapp";
export type Stage = "downloading" | "verifying" | "extracting";

/** Mirrors `Job`. */
export interface Job {
  id: string;
  version: string;
  stage: Stage | null;
  received: number;
  total: number | null;
  error: string | null;
  done: boolean;
}

/** Mirrors `InstalledVersion`. */
export interface InstalledVersion {
  version: string;
  /** "running" or "selected" when it cannot be removed. */
  inUse: "running" | "selected" | null;
  managed: boolean;
  requiresPhp: string | null;
}

/** Mirrors `AvailableVersion`. */
export interface AvailableVersion {
  version: string;
  channel: string | null;
  size: number | null;
  requiresPhp: string | null;
  installed: boolean;
  partial: number | null;
  compatible: boolean;
}

/** Mirrors `PackageView`. */
export interface PackageView {
  id: string;
  name: string;
  kind: PackageKind;
  description: string;
  homepage: string;
  selected: string;
  active: string | null;
  running: string | null;
  installed: InstalledVersion[];
  available: AvailableVersion[];
  jobs: Job[];
}

/** Mirrors `PackagesOverview`. */
export interface PackagesOverview {
  packages: PackageView[];
  phpVersion: string | null;
  dbUi: string | null;
  catalogUpdated: string;
}

/** Payload of the `package-progress` event. */
export interface ProgressEvent {
  id: string;
  version: string;
  stage: Stage;
  received: number;
  total: number | null;
}

export const PACKAGES_KEY = ["packages"] as const;

export const packagesList = () => call<PackagesOverview>("packages_list");
export const packageInstall = (id: string, version: string) => call<void>("package_install", { id, version });
export const packageCancel = (id: string, version: string) => call<void>("package_cancel", { id, version });
export const packageDismiss = (id: string, version: string) => call<void>("package_dismiss", { id, version });
export const packageUninstall = (id: string, version: string) => call<void>("package_uninstall", { id, version });
/** `version` "" = newest installed (phpMyAdmin: newest that runs on the active PHP). */
export const packageSetActive = (id: string, version: string) => call<void>("package_set_active", { id, version });
export const dbUiOpen = () => call<void>("db_ui_open");

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
