import type { WebServer } from "@/features/services/api";
import { call } from "@/lib/ipc";
import type { ConfigWarning } from "@/types";

/** Mirrors lumina_config::schema (field names are snake_case, as in config.toml). */
export interface GeneralSettings {
  www_root: string;
  tld: string;
  auto_vhosts: boolean;
  auto_ssl: boolean;
  start_on_login: boolean;
  autostart_services: boolean;
  close_to_tray: boolean;
}

export interface PhpSettings {
  version: string;
  fcgi_port: number;
  fcgi_fallback_port: number;
  fcgi_children: number;
  fcgi_max_requests: number;
}

export interface WebSettings {
  server: WebServer;
  nginx_version: string;
  apache_version: string;
  http_port: number;
  https_port: number;
}

export interface EditorSettings {
  name: string;
  command: string;
  args: string[];
}

export interface AppConfig {
  schema_version: number;
  general: GeneralSettings;
  php: PhpSettings;
  web: WebSettings;
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
