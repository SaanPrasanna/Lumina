import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Event names must match apps/desktop/src-tauri/src/events.rs. */
export type BackendEvent = "service-status" | "log-line" | "download-progress";

export function onBackendEvent<T>(event: BackendEvent, cb: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => cb(e.payload));
}
