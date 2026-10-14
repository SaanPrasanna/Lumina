// IPC wrappers for the `quickapp` feature (see src-tauri/src/commands/quickapp.rs).
import { call } from "@/lib/ipc";

export type Need = "php" | "composer" | "node" | "database";
export type JobState = "running" | "succeeded" | "failed" | "cancelled";

export interface Choice {
  value: string;
  label: string;
  needs: Need[];
}

export interface RecipeOption {
  id: string;
  label: string;
  default: string;
  choices: Choice[];
}

/** Mirrors `RecipeView`. */
export interface Recipe {
  id: string;
  name: string;
  description: string;
  homepage: string;
  category: string;
  needs: Need[];
  options: RecipeOption[];
}

export interface Step {
  index: number;
  total: number;
  label: string;
}

/** Mirrors `Job`. */
export interface Job {
  id: number;
  recipe: string;
  recipeName: string;
  name: string;
  dir: string;
  domain: string;
  url: string;
  state: JobState;
  step: Step | null;
  /** [received, total] bytes of a running download. */
  download: [number, number | null] | null;
  error: string | null;
  /** Created, but e.g. the web server could not start. */
  warning: string | null;
  lines: string[];
  /** Lines dropped from the front of `lines`. */
  dropped: number;
  partial: boolean;
  createdDatabase: string | null;
  logFile: string;
}

export interface DatabaseInfo {
  engine: "mysql" | "mariadb";
  host: string;
  port: number;
  user: string;
  password: string;
}

/** Mirrors `QuickAppOverview`. */
export interface QuickAppOverview {
  recipes: Recipe[];
  www: string;
  tld: string;
  missing: Need[];
  taken: string[];
  job: Job | null;
  database: DatabaseInfo;
}

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

export const quickappOverview = () => call<QuickAppOverview>("quickapp_overview");
export const quickappCreate = (recipe: string, name: string, options: Record<string, string>) =>
  call<void>("quickapp_create", { recipe, name, options });
export const quickappCancel = () => call<void>("quickapp_cancel");
export const quickappDismiss = () => call<void>("quickapp_dismiss");
export const quickappDeletePartial = () => call<void>("quickapp_delete_partial");

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
