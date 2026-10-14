// IPC wrappers for the `profiles` feature (see src-tauri/src/commands/profiles.rs).
import type { DatabaseEngine, WebServer } from "@/features/services/api";
import { call } from "@/lib/ipc";
import type { ServiceKind } from "@/types";

/** Mirrors lumina_config::schema::Profile (snake_case, as in config.toml). */
export interface Profile {
  name: string;
  web: WebServer;
  /** "" = newest installed. */
  web_version: string;
  php: string;
  database: { engine: DatabaseEngine; version: string };
  redis: boolean;
}

export interface ProfileView extends Profile {
  current: boolean;
  missing: string[];
}

export interface ProfilesOverview {
  profiles: ProfileView[];
  stack: Profile;
  active: string;
}

/** Mirrors lumina_services::profiles::Plan. */
export interface SwitchPlan {
  stop: ServiceKind[];
  start: ServiceKind[];
  keep: ServiceKind[];
}

export const PROFILES_KEY = ["profiles"] as const;

export const profilesList = () => call<ProfilesOverview>("profiles_list");
/** Adds `profile`, or replaces the one named `original`. */
export const profileSave = (profile: Profile, original: string | null) => call<void>("profile_save", { profile, original });
export const profileSaveCurrent = (name: string) => call<void>("profile_save_current", { name });
export const profileDelete = (name: string) => call<void>("profile_delete", { name });
export const profileSwitch = (name: string) => call<SwitchPlan>("profile_switch", { name });

/** "nginx · PHP 8.3 · MySQL 8.4 · Redis". */
export function describe(p: Profile, newest: string): string {
  const v = (version: string) => (version ? version : newest);
  const parts = [
    `${p.web === "nginx" ? "nginx" : "Apache"} ${v(p.web_version)}`,
    `PHP ${v(p.php)}`,
    `${p.database.engine === "mysql" ? "MySQL" : "MariaDB"} ${v(p.database.version)}`,
  ];
  if (p.redis) parts.push("Redis");
  return parts.join(" · ");
}
