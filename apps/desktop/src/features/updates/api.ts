// IPC wrappers for app updates (see src-tauri/src/commands/updates.rs).
import { commands } from "@/lib/bindings";

export type { UpdateInfo, UpdateProgress, UpdateStatus } from "@/lib/bindings";

export const UPDATES_KEY = ["updates"] as const;
export type Channel = "stable" | "beta";

export const updatesStatus = () => commands.updatesStatus();
/** Asks the release server now. */
export const updatesCheck = () => commands.updatesCheck();
export const updatesConfigure = (channel: Channel, check: boolean) => commands.updatesConfigure(channel, check);
/** Stops the services, installs the verified update and restarts Lumina. */
export const updatesInstall = () => commands.updatesInstall();
