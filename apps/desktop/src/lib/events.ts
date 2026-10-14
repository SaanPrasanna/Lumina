import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";

/** Event names must match apps/desktop/src-tauri/src/events.rs. */
export type BackendEvent = "service-status" | "config-changed";

/** Query keys refetched when each backend event arrives. */
const INVALIDATES: Record<BackendEvent, readonly (readonly string[])[]> = {
  "service-status": [["php"]],
  "config-changed": [["settings"], ["php"]],
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
