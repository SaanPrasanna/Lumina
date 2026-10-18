import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";
import { DashboardPage } from "@/features/dashboard";
import * as services from "@/features/services/api";

vi.mock("@/features/onboarding/api", async (orig) => ({
  ...(await orig<typeof import("@/features/onboarding/api")>()),
  bootstrapStatus: vi.fn().mockResolvedValue({ configured: true, dataRoot: "C:\\Lumina", defaultDataRoot: "C:\\Lumina", language: "en" }),
}));
vi.mock("@/features/settings/api", async (orig) => ({
  ...(await orig<typeof import("@/features/settings/api")>()),
  settingsGet: vi.fn().mockResolvedValue({ config: {}, warnings: [], source: "file", path: "x" }),
}));
vi.mock("@/features/sites/api", async (orig) => ({
  ...(await orig<typeof import("@/features/sites/api")>()),
  sitesList: vi.fn().mockResolvedValue({ sites: [{ name: "blog" }, { name: "shop" }], tld: "test" }),
}));
vi.mock("@/features/tunnel/api", async (orig) => ({
  ...(await orig<typeof import("@/features/tunnel/api")>()),
  tunnelOverview: vi.fn().mockResolvedValue({ tunnels: [{ name: "blog" }] }),
}));
vi.mock("@/features/services/api", async (orig) => ({
  ...(await orig<typeof services>()),
  servicesStatus: vi.fn(),
  serviceStart: vi.fn(),
  serviceStop: vi.fn(),
  postgresqlVersions: vi.fn().mockResolvedValue([]),
  redisVersions: vi.fn().mockResolvedValue([{ version: "8.10.2" }]),
}));

const stack = {
  php: { status: "running", runtime: { version: "8.4.26", port: 10987 }, error: null },
  web: { server: "nginx", status: "running", runtime: { version: "1.30.5", httpPort: 8080 }, error: null },
  database: { engine: "mysql", status: "stopped", runtime: null, error: null },
  redis: { status: "failed", runtime: null, error: "port 6379 is already in use" },
  postgresql: { status: "stopped", runtime: null, error: null },
  mailpit: { status: "stopped", runtime: null, error: null },
  phpPools: [],
} as unknown as services.StackStatus;

describe("DashboardPage", () => {
  it("shows every service with its state and starts a stopped one", async () => {
    vi.mocked(services.servicesStatus).mockResolvedValue(stack);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <MemoryRouter>
          <DashboardPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    expect(await screen.findByText("8.4.26")).toBeInTheDocument();
    expect(screen.getByText("port 8080")).toBeInTheDocument();
    expect(screen.getByText("port 6379 is already in use")).toBeInTheDocument();
    expect(screen.getAllByText("Running")).toHaveLength(2);

    await userEvent.click(screen.getByRole("button", { name: "Start MySQL" }));
    expect(services.serviceStart).toHaveBeenCalledWith("mysql");
    await userEvent.click(screen.getByRole("button", { name: "Stop nginx" }));
    expect(services.serviceStop).toHaveBeenCalledWith("nginx");

    // Sites and shares at a glance, linking to their pages.
    expect(await screen.findByRole("link", { name: /Sites\s*2/ })).toHaveAttribute("href", "/sites");
    expect(await screen.findByRole("link", { name: /Shared now\s*1/ })).toHaveAttribute("href", "/tunnel");
    expect(screen.getByText("C:\\Lumina")).toBeInTheDocument();
  });

  it("marks what is not installed and links to Quick Add", async () => {
    vi.mocked(services.servicesStatus).mockResolvedValue(stack);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <MemoryRouter>
          <DashboardPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    const install = await screen.findByRole("link", { name: "Install PostgreSQL" });
    expect(install).toHaveAttribute("href", "#/packages?package=postgresql");
    expect(screen.getByText("Not installed")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Start PostgreSQL" })).not.toBeInTheDocument();
    // Redis is installed: it keeps its Start button.
    expect(screen.getByRole("button", { name: "Start Redis" })).toBeInTheDocument();
  });
});
