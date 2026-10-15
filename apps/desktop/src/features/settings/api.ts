// IPC wrappers for config.toml (see src-tauri/src/commands/settings.rs).
// Field names are snake_case, as in config.toml.
import { commands } from "@/lib/bindings";
import type * as B from "@/lib/bindings";

// The backend always sends complete sections. The generated types mark their
// fields optional because the Rust structs have `#[serde(default)]` (missing
// keys are filled in when config.toml is read), so the UI states the shape it
// actually receives.
export type GeneralSettings = B.General_Serialize;
export type PhpSettings = Required<B.Php>;
export type WebSettings = Required<B.Web>;
export type DatabaseSettings = Required<B.Database>;
export type RedisSettings = Required<B.Redis>;
export type EditorSettings = Required<B.Editor>;
export type SiteOverride = Required<B.SiteOverride>;
export type AppConfig = Omit<B.AppConfig_Serialize, "php" | "web" | "database" | "redis" | "editor" | "tunnel" | "tools" | "packages" | "sites" | "updates"> & {
  php: PhpSettings;
  web: WebSettings;
  database: DatabaseSettings;
  redis: RedisSettings;
  editor: EditorSettings;
  tunnel: Required<B.Tunnel>;
  tools: Required<B.Tools>;
  packages: Required<B.Packages>;
  updates: Required<B.Updates>;
  sites: Record<string, SiteOverride>;
};
export type Settings = Omit<B.Settings_Serialize, "config"> & { config: AppConfig };

export const SETTINGS_KEY = ["settings"] as const;

export const settingsGet = () => commands.settingsGet() as Promise<Settings>;

/** Replaces one section of config.toml; the backend validates the result. */
export const settingsUpdate = (section: string, value: unknown) => commands.settingsUpdate(section, value) as Promise<Settings>;
