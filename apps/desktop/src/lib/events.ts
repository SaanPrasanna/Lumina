import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";

/** Event names must match apps/desktop/src-tauri/src/events.rs. */
export type BackendEvent =
  | "service-status"
  | "config-changed"
  | "sites-changed"
  | "packages-changed"
  | "quickapp-changed";

/** Query keys refetched when each backend event arrives. */
const INVALIDATES: Record<BackendEvent, readonly (readonly string[])[]> = {
  "service-status": [["services"], ["sites"], ["profiles"]],
  "config-changed": [["settings"], ["services"], ["sites"], ["profiles"]],
  "sites-changed": [["sites"]],
  "packages-changed": [["packages"], ["php"], ["web"], ["db"], ["redis"], ["services"], ["profiles"], ["quickapp"]],
  "quickapp-changed": [["quickapp"], ["sites"], ["services"]],
};

/** Keeps React Query data fresh by listening to backend events. Mount once. */
export function useBackendEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    const unlisteners = (Object.keys(INVALIDATES) as BackendEvent[]).map((event) =>
      listen(event, () => {
        for (const queryKey of INVALIDATES[event]) {
          void queryClient.invalidateQueries({ queryKey });
        }
      }),
    );
    return () => {
      for (const unlisten of unlisteners) void unlisten.then((fn) => fn());
    };
  }, [queryClient]);
}
