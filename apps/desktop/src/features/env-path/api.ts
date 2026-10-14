// IPC wrappers for the `env-path` feature and the editor launcher
// (see src-tauri/src/commands/env_path.rs and editor.rs).
import { call } from "@/lib/ipc";

/** Mirrors lumina_services::tools::Shim. */
export interface Shim {
  name: string;
  program: string;
  args: string[];
  describes: string;
}

export interface Shadow {
  command: string;
  found: string;
}

/** Mirrors `PathOverview`. */
export interface PathOverview {
  shimDir: string;
  added: boolean;
  /** Also on the machine-wide PATH (Windows). */
  addedSystem: boolean;
  files: string[];
  shadowed: Shadow[];
  shims: Shim[];
  nativeShims: boolean;
}

export const PATH_KEY = ["path"] as const;

export const pathStatus = () => call<PathOverview>("path_status");
export const pathAdd = () => call<PathOverview>("path_add");
export const pathRemove = () => call<PathOverview>("path_remove");
/** Windows: put Lumina first on the machine-wide PATH too (one administrator prompt). */
export const pathAddSystem = () => call<PathOverview>("path_add_system");
/** Opens a terminal (default: the projects folder) with Lumina's commands on PATH. */
export const terminalOpen = (path?: string) => call<void>("terminal_open", { path: path ?? null });

/** Mirrors lumina_platform::launcher::EditorPreset. */
export interface EditorPreset {
  id: string;
  name: string;
  command: string;
  args: string[];
  argsLine: string[];
}

export type ConfigFile = "lumina" | "phpIni" | "database" | "redis" | "webSites";
export type OpenTarget = { kind: "site"; name: string } | { kind: "config"; file: ConfigFile } | { kind: "www" };

export const editorPresets = () => call<EditorPreset[]>("editor_presets");
export const editorOpen = (target: OpenTarget, line?: number) =>
  call<void>("editor_open", { target, line: line ?? null });
