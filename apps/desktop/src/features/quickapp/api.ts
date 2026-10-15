// IPC wrappers for the `quickapp` feature (see src-tauri/src/commands/quickapp.rs).
import { commands } from "@/lib/bindings";
import type { Choice_Serialize, QuickAppOverview_Serialize, RecipeOption_Serialize, RecipeView_Serialize, Step } from "@/lib/bindings";

export type { DatabaseInfo, JobState, Need, QuickAppJob as Job, Step } from "@/lib/bindings";
export type Choice = Choice_Serialize;
export type RecipeOption = RecipeOption_Serialize;
export type Recipe = RecipeView_Serialize;
export type QuickAppOverview = QuickAppOverview_Serialize;

/** Payload of the `quickapp-output` event. */
export interface OutputEvent {
  jobId: number;
  /** Index of `lines[0]` in the job's whole output. */
  first: number;
  lines: string[];
  step: Step | null;
  download: [number, number | null] | null;
}

export const QUICKAPP_KEY = ["quickapp"] as const;

export const quickappOverview = () => commands.quickappOverview();
export const quickappCreate = (recipe: string, name: string, options: Record<string, string>) =>
  commands.quickappCreate(recipe, name, options);
export const quickappCancel = () => commands.quickappCancel();
export const quickappDismiss = () => commands.quickappDismiss();
export const quickappDeletePartial = () => commands.quickappDeletePartial();

/** Same rule as `files::is_valid_name`: it becomes the folder, the domain and the database. */
export function isValidName(name: string): boolean {
  return /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(name);
}

/** "My Blog_2" → "my-blog-2" while typing (a trailing dash is kept so typing can continue). */
export function toName(input: string): string {
  return input
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, 63);
}

/** `base`, or `base-2`, `base-3`... if taken. */
export function freeName(base: string, taken: string[]): string {
  if (!taken.includes(base)) return base;
  for (let i = 2; ; i++) {
    const name = `${base}-${i}`;
    if (!taken.includes(name)) return name;
  }
}
