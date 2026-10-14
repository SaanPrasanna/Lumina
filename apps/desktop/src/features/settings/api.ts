import { call } from "@/lib/ipc";
import type { ConfigWarning } from "@/types";

/** Mirrors lumina_config::schema (field names are snake_case, as in config.toml). */
export interface PhpSettings {
  version: string;
  fcgi_port: number;
  fcgi_fallback_port: number;
  fcgi_children: number;
  fcgi_max_requests: number;
}

export interface EditorSettings {
  name: string;
  command: string;
  args: string[];
}

export interface AppConfig {
  schema_version: number;
  php: PhpSettings;
  editor: EditorSettings;
  // Other sections are edited in later phases.
  [section: string]: unknown;
}

/** Mirrors `Settings` in src-tauri/src/commands/settings.rs. */
export interface Settings {
  config: AppConfig;
  warnings: ConfigWarning[];
  source: "file" | "created" | "restored-from-backup" | "reset-to-defaults";
  path: string;
}

export const SETTINGS_KEY = ["settings"] as const;

export const settingsGet = () => call<Settings>("settings_get");

export const settingsUpdate = (section: string, value: unknown) =>
  call<Settings>("settings_update", { section, value });
