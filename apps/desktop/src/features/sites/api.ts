// IPC wrappers for the `sites` feature (see src-tauri/src/commands/sites.rs).
import { call } from "@/lib/ipc";

/** Mirrors lumina_vhost::sites::DocrootSource. */
export type DocrootSource = { kind: "detected"; subdir: string | null } | { kind: "configured" };

/** Mirrors lumina_vhost::sites::Site. */
export interface Site {
  name: string;
  domain: string;
  path: string;
  docroot: string;
  docrootSource: DocrootSource;
  docrootExists: boolean;
}

/** Mirrors lumina_vhost::sites::SkipReason. */
export type SkipReason =
  | { kind: "invalidName" }
  | { kind: "duplicateDomain"; domain: string; takenBy: string }
  | { kind: "disabled" }
  | { kind: "unsafePath" };

export interface Skipped {
  name: string;
  reason: SkipReason;
}

/** Mirrors lumina_vhost::sync::Attempt. */
export interface HostsAttempt {
  outcome: "upToDate" | "updated" | "declined" | "helperMissing" | "failed";
  message: string | null;
}

/** Mirrors lumina_vhost::sync::HostsReport. */
export interface HostsReport {
  file: string;
  inSync: boolean;
  missing: string[];
  extra: string[];
  pending: boolean;
  last: HostsAttempt | null;
  error: string | null;
}

/** Mirrors `SitesOverview` in src-tauri/src/commands/sites.rs. */
export interface SitesOverview {
  sites: Site[];
  skipped: Skipped[];
  wwwRoot: string;
  tld: string;
  autoVhosts: boolean;
  webRunning: boolean;
  httpPort: number;
  /** Set when HTTPS is on. */
  httpsPort: number | null;
  hosts: HostsReport;
}

export const SITES_KEY = ["sites"] as const;

export const sitesList = () => call<SitesOverview>("sites_list");

/** Applies the projects folder now; asks for administrator rights if the hosts file needs it. */
export const sitesSync = () => call<HostsAttempt | null>("sites_sync");

export const siteOpen = (domain: string) => call<void>("site_open", { domain });

export const siteOpenFolder = (name: string) => call<void>("site_open_folder", { name });
