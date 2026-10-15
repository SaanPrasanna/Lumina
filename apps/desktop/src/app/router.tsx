import { createHashRouter, RouterProvider } from "react-router";
import { NAV_ITEMS } from "@/app/nav";
import { AppShell } from "@/components/layout/AppShell";
import { ComingSoonPage } from "@/components/layout/ComingSoonPage";
import { DashboardPage } from "@/features/dashboard";
import { EnvPathPage } from "@/features/env-path";
import { PackagesPage } from "@/features/packages";
import { ProfilesPage } from "@/features/profiles";
import { QuickAppPage } from "@/features/quickapp";
import { ServicesPage } from "@/features/services";
import { SettingsPage } from "@/features/settings";
import { SitesPage } from "@/features/sites";
import { TunnelPage } from "@/features/tunnel";

/** Implemented pages; every other nav item shows a placeholder for now. */
const PAGES: Record<string, React.ReactNode> = {
  "/services": <ServicesPage />,
  "/sites": <SitesPage />,
  "/packages": <PackagesPage />,
  "/profiles": <ProfilesPage />,
  "/quickapp": <QuickAppPage />,
  "/tunnel": <TunnelPage />,
  "/env-path": <EnvPathPage />,
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
