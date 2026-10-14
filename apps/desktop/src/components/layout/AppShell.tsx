import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { Outlet, useNavigate } from "react-router";
import { Sidebar } from "@/components/layout/Sidebar";

/**
 * Fixed-width sidebar + scrollable content. Fixed sizes, `min-w-0` and no layout
 * transitions keep the UI from shaking while the window is resized.
 */
export function AppShell() {
  const navigate = useNavigate();

  // The tray asks the window to show a page (e.g. Services after a failed start).
  useEffect(() => {
    const unlisten = listen<string>("navigate", (e) => navigate(e.payload));
    return () => void unlisten.then((fn) => fn());
  }, [navigate]);

  return (
    <div className="flex h-full overflow-hidden bg-surface text-fg">
      <Sidebar />
      <main className="min-w-0 flex-1 overflow-y-auto">
        <Outlet />
      </main>
    </div>
  );
}
