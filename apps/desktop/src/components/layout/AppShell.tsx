import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { Outlet, useNavigate } from "react-router";
import { Sidebar } from "@/components/layout/Sidebar";
import { UpdateBanner } from "@/features/updates/UpdatesCard";

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
      {/* The scroll container keeps room for its scrollbar, so pages that grow
          past the window height do not shift sideways. */}
      <main className="min-w-0 flex-1 overflow-y-auto [scrollbar-gutter:stable]">
        <UpdateBanner
          onOpen={() => {
            navigate("/settings");
            // After the page has rendered.
            window.setTimeout(() => document.getElementById("updates")?.scrollIntoView({ block: "start" }), 100);
          }}
        />
        <Outlet />
      </main>
    </div>
  );
}
