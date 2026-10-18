import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { Outlet, useLocation, useNavigate } from "react-router";
import { Sidebar } from "@/components/layout/Sidebar";
import { UpdateBanner } from "@/features/updates/UpdatesCard";
import { commands } from "@/lib/bindings";

/**
 * Fixed-width sidebar + scrollable content. Fixed sizes, `min-w-0` and no layout
 * transitions keep the UI from shaking while the window is resized.
 */
export function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();

  // The tray asks the window to show a page (e.g. Services after a failed start).
  useEffect(() => {
    const unlisten = listen<string>("navigate", (e) => navigate(e.payload));
    return () => void unlisten.then((fn) => fn());
  }, [navigate]);

  // Right-click shows Lumina's menu (the tray's) instead of the browser's.
  // Text fields and selected text keep the native Cut/Copy/Paste menu.
  useEffect(() => {
    const onContextMenu = (e: MouseEvent) => {
      const target = e.target instanceof Element ? e.target : null;
      if (target?.closest("input, textarea, [contenteditable='true']")) return;
      if (window.getSelection()?.toString()) return;
      e.preventDefault();
      void commands.contextMenuShow();
    };
    document.addEventListener("contextmenu", onContextMenu);
    return () => document.removeEventListener("contextmenu", onContextMenu);
  }, []);

  return (
    <div className="relative flex h-full overflow-hidden bg-surface text-fg">
      <Sidebar />
      {/* The scroll container keeps room for its scrollbar, so pages that grow
          past the window height do not shift sideways. */}
      <main className="relative min-w-0 flex-1 overflow-y-auto overscroll-y-contain [scrollbar-gutter:stable]">
        <UpdateBanner
          onOpen={() => {
            navigate("/settings");
            // After the page has rendered.
            window.setTimeout(() => document.getElementById("updates")?.scrollIntoView({ block: "start" }), 100);
          }}
        />
        {/* Each page fades in (opacity only: nothing moves while resizing). */}
        <div key={location.pathname} className="animate-fade-in">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
