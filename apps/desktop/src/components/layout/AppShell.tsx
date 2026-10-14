import { Outlet } from "react-router";

/** Fixed sidebar + scrollable content. Fixed sizes avoid layout shake on resize. */
export function AppShell() {
  return (
    <div className="flex h-screen overflow-hidden">
      <aside className="w-56 shrink-0 border-e" />
      <main className="min-w-0 flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
