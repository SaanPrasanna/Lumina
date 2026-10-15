// IPC wrappers for the `env-path` feature and the editor launcher
// (see src-tauri/src/commands/env_path.rs and editor.rs).
import { commands } from "@/lib/bindings";
import type { OpenTarget } from "@/lib/bindings";

export type { ConfigFile, EditorPreset, OpenTarget, PathOverview, Shadow, Shim } from "@/lib/bindings";

export const PATH_KEY = ["path"] as const;

export const pathStatus = () => commands.pathStatus();
export const pathAdd = () => commands.pathAdd();
export const pathRemove = () => commands.pathRemove();
/** Windows: put Lumina first on the machine-wide PATH too (one administrator prompt). */
export const pathAddSystem = () => commands.pathAddSystem();
/** Opens a terminal (default: the projects folder) with Lumina's commands on PATH. */
export const terminalOpen = (path?: string) => commands.terminalOpen(path ?? null);

export const editorPresets = () => commands.editorPresets();
export const editorOpen = (target: OpenTarget, line?: number) => commands.editorOpen(target, line ?? null);
