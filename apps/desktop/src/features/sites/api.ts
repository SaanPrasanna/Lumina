// IPC wrappers for the `sites` feature (see src-tauri/src/commands/sites.rs).
import { commands } from "@/lib/bindings";
import type { Attempt } from "@/lib/bindings";

export type { DocrootSource, HostsReport, Site, SitesOverview, SkipReason, Skipped } from "@/lib/bindings";
export type HostsAttempt = Attempt;

export const SITES_KEY = ["sites"] as const;

export const sitesList = () => commands.sitesList();

/** Applies the projects folder now; asks for administrator rights if the hosts file needs it. */
export const sitesSync = () => commands.sitesSync();

export const siteOpen = (domain: string) => commands.siteOpen(domain);

export const siteOpenFolder = (name: string) => commands.siteOpenFolder(name);
