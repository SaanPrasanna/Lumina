// IPC wrappers for the `sites` feature (see src-tauri/src/commands/sites.rs).
import { commands } from "@/lib/bindings";
import type { Attempt } from "@/lib/bindings";

export type { AppSite, AppStatus, DocrootSource, Hook, LanOverview, Request, WebhooksStatus, HostsReport, Site, SitesOverview, SkipReason, Skipped } from "@/lib/bindings";
export type HostsAttempt = Attempt;

export const SITES_KEY = ["sites"] as const;
/** Under SITES_KEY, so service-status events refresh it too. */
export const APPS_KEY = ["sites", "apps"] as const;

export const sitesList = () => commands.sitesList();

/** Applies the projects folder now; asks for administrator rights if the hosts file needs it. */
export const sitesSync = () => commands.sitesSync();

export const siteOpen = (domain: string) => commands.siteOpen(domain);

export const siteOpenFolder = (name: string) => commands.siteOpenFolder(name);

/** Dev servers of sites served by one (Vite, Next.js, Django, ...). */
export const appsStatus = () => commands.appsStatus();
export const appStart = (name: string) => commands.appStart(name);
export const appStop = (name: string) => commands.appStop(name);
export const appRestart = (name: string) => commands.appRestart(name);
export const appOpenLog = (name: string) => commands.appOpenLog(name);
/** Writes .vscode/launch.json for Xdebug (if missing); returns its path. */
export const siteXdebugConfig = (name: string) => commands.siteXdebugConfig(name);
/** The newest requests to a site, newest first. */
export const siteRequests = (name: string, limit: number) => commands.siteRequests(name, limit);
/** npx playwright install in the project (browsers for its tests). */
export const sitePlaywrightInstall = (name: string) => commands.sitePlaywrightInstall(name);

/** This computer's network address and every site's address there. */
export const LAN_KEY = ["sites", "lan"] as const;
export const lanOverview = () => commands.lanOverview();

export const WEBHOOKS_KEY = ["sites", "webhooks"] as const;
export const webhooksStatus = () => commands.webhooksStatus();
export const webhooksStart = () => commands.webhooksStart();
export const webhooksStop = () => commands.webhooksStop();
export const webhooksClear = () => commands.webhooksClear();
export const webhookReplay = (id: number, site: string) => commands.webhookReplay(id, site);
