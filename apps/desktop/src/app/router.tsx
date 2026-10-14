import { createHashRouter, RouterProvider } from "react-router";
import { AppShell } from "@/components/layout/AppShell";
import { DashboardPage } from "@/features/dashboard";

const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { index: true, element: <DashboardPage /> },
      // services, sites, packages, profiles, quickapp, tunnel, settings, logs …
    ],
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
