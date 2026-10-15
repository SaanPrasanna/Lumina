// IPC wrappers for the `profiles` feature (see src-tauri/src/commands/profiles.rs).
import { commands } from "@/lib/bindings";
import type * as B from "@/lib/bindings";

export type { Plan as SwitchPlan } from "@/lib/bindings";
// Complete as sent by the backend (the Rust struct has `#[serde(default)]`,
// which makes the generated fields optional; see settings/api.ts).
export type Profile = Required<Omit<B.Profile, "database">> & { database: Required<B.ProfileDatabase> };
export type ProfileView = Omit<B.ProfileView, keyof B.Profile> & Profile;
export type ProfilesOverview = Omit<B.ProfilesOverview, "profiles" | "stack"> & { profiles: ProfileView[]; stack: Profile };

export const PROFILES_KEY = ["profiles"] as const;

export const profilesList = () => commands.profilesList() as Promise<ProfilesOverview>;
/** Adds `profile`, or replaces the one named `original`. */
export const profileSave = (profile: Profile, original: string | null) => commands.profileSave(profile, original);
export const profileSaveCurrent = (name: string) => commands.profileSaveCurrent(name);
export const profileDelete = (name: string) => commands.profileDelete(name);
export const profileSwitch = (name: string) => commands.profileSwitch(name);

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
