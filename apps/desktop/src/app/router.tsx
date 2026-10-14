import { createHashRouter, RouterProvider } from "react-router";
import { NAV_ITEMS } from "@/app/nav";
import { AppShell } from "@/components/layout/AppShell";
import { ComingSoonPage } from "@/components/layout/ComingSoonPage";
import { DashboardPage } from "@/features/dashboard";

// Hash routing: the app is served from a custom protocol, so there is no server
// to rewrite deep links.
const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { index: true, element: <DashboardPage /> },
      // Feature pages replace these placeholders phase by phase.
      ...NAV_ITEMS.filter((item) => item.path !== "/").map((item) => ({
        path: item.path,
        element: <ComingSoonPage titleKey={item.labelKey} />,
      })),
    ],
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
