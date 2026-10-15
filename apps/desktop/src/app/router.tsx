import { lazy, Suspense } from "react";
import { createHashRouter, RouterProvider } from "react-router";
import { NAV_ITEMS } from "@/app/nav";
import { AppShell } from "@/components/layout/AppShell";
import { ComingSoonPage } from "@/components/layout/ComingSoonPage";
import { DashboardPage } from "@/features/dashboard";

// The dashboard opens first and is bundled; every other page is its own
// chunk, loaded on first visit (local files, so it is instant).
const ServicesPage = lazy(() => import("@/features/services").then((m) => ({ default: m.ServicesPage })));
const SitesPage = lazy(() => import("@/features/sites").then((m) => ({ default: m.SitesPage })));
const PackagesPage = lazy(() => import("@/features/packages").then((m) => ({ default: m.PackagesPage })));
const ProfilesPage = lazy(() => import("@/features/profiles").then((m) => ({ default: m.ProfilesPage })));
const QuickAppPage = lazy(() => import("@/features/quickapp").then((m) => ({ default: m.QuickAppPage })));
const TunnelPage = lazy(() => import("@/features/tunnel").then((m) => ({ default: m.TunnelPage })));
const EnvPathPage = lazy(() => import("@/features/env-path").then((m) => ({ default: m.EnvPathPage })));
const LogsPage = lazy(() => import("@/features/logs").then((m) => ({ default: m.LogsPage })));
const SettingsPage = lazy(() => import("@/features/settings").then((m) => ({ default: m.SettingsPage })));

/** Implemented pages; every other nav item shows a placeholder. */
const PAGES: Record<string, React.ReactNode> = {
  "/services": <ServicesPage />,
  "/sites": <SitesPage />,
  "/packages": <PackagesPage />,
  "/profiles": <ProfilesPage />,
  "/quickapp": <QuickAppPage />,
  "/tunnel": <TunnelPage />,
  "/env-path": <EnvPathPage />,
  "/logs": <LogsPage />,
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
        element: <Suspense fallback={null}>{PAGES[item.path] ?? <ComingSoonPage titleKey={item.labelKey} />}</Suspense>,
      })),
    ],
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
