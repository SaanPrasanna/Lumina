import { Outlet } from "react-router";
import { Sidebar } from "@/components/layout/Sidebar";

/**
 * Fixed-width sidebar + scrollable content. Fixed sizes, `min-w-0` and no layout
 * transitions keep the UI from shaking while the window is resized.
 */
export function AppShell() {
  return (
    <div className="flex h-full overflow-hidden bg-surface text-fg">
      <Sidebar />
      <main className="min-w-0 flex-1 overflow-y-auto">
        <Outlet />
      </main>
    </div>
  );
}
