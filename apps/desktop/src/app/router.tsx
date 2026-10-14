import { createHashRouter, RouterProvider } from "react-router";
import { NAV_ITEMS } from "@/app/nav";
import { AppShell } from "@/components/layout/AppShell";
import { ComingSoonPage } from "@/components/layout/ComingSoonPage";
import { DashboardPage } from "@/features/dashboard";
import { ServicesPage } from "@/features/services";
import { SettingsPage } from "@/features/settings";
import { SitesPage } from "@/features/sites";

/** Implemented pages; every other nav item shows a placeholder for now. */
const PAGES: Record<string, React.ReactNode> = {
  "/services": <ServicesPage />,
  "/sites": <SitesPage />,
  "/settings": <SettingsPage />,
};

// Hash routing: the app is served from a custom protocol, so there is no server
// to rewrite deep links.
const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { index: true, element: <DashboardPage /> },
      ...NAV_ITEMS.filter((item) => item.path !== "/").map((item) => ({
        path: item.path,
        element: PAGES[item.path] ?? <ComingSoonPage titleKey={item.labelKey} />,
      })),
    ],
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
