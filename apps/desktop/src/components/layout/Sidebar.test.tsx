import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { describe, expect, it, vi } from "vitest";
import { NAV_ITEMS } from "@/app/nav";
import { Sidebar } from "@/components/layout/Sidebar";

vi.mock("@/features/onboarding/api", () => ({
  BOOTSTRAP_QUERY_KEY: ["bootstrap"],
  setLanguage: vi.fn(),
}));

function renderAt(path: string) {
  const router = createMemoryRouter([{ path: "*", element: <Sidebar /> }], {
    initialEntries: [path],
  });
  render(
    <QueryClientProvider client={new QueryClient()}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

describe("Sidebar", () => {
  it("renders a link for every nav item", () => {
    renderAt("/");
    expect(screen.getAllByRole("link")).toHaveLength(NAV_ITEMS.length);
    expect(screen.getByRole("link", { name: "Quick Add" })).toHaveAttribute("href", "/packages");
  });

  it("marks only the current route as active", () => {
    renderAt("/sites");
    expect(screen.getByRole("link", { name: "Sites" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Dashboard" })).not.toHaveAttribute("aria-current");
  });
});
