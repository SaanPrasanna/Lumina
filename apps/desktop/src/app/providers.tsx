import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const queryClient = new QueryClient({
  defaultOptions: {
    // Backend state changes are pushed via events; no polling or refetch-on-focus.
    queries: { retry: false, refetchOnWindowFocus: false, staleTime: Infinity },
  },
});

/** Global providers. Theme and text direction are applied directly to <html>. */
export function AppProviders({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}
