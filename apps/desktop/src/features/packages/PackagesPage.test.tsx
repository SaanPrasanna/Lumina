import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { PackagesPage } from "@/features/packages";
import * as api from "@/features/packages/api";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("@/features/packages/api", async (orig) => ({
  ...(await orig<typeof api>()),
  packagesList: vi.fn(),
  packageInstall: vi.fn(),
  packageUninstall: vi.fn(),
  packageSetActive: vi.fn(),
  packageCancel: vi.fn(),
}));

const php: api.PackageView = {
  id: "php",
  name: "PHP",
  kind: "service",
  description: "PHP builds",
  homepage: "",
  selected: "",
  active: "8.4.26",
  running: "8.4.26",
  installed: [
    { version: "8.4.26", inUse: "running", managed: false, requiresPhp: null },
    { version: "8.3.35", inUse: null, managed: true, requiresPhp: null },
  ],
  available: [
    { version: "8.5.11", channel: null, size: 36_000_000, requiresPhp: null, installed: false, partial: null, compatible: true },
    { version: "8.4.26", channel: null, size: null, requiresPhp: null, installed: true, partial: null, compatible: true },
  ],
  jobs: [],
};

const pma: api.PackageView = {
  ...php,
  id: "phpmyadmin",
  name: "phpMyAdmin",
  kind: "webapp",
  active: null,
  running: null,
  installed: [],
  available: [
    { version: "5.2.3", channel: null, size: null, requiresPhp: ">=7.2.5", installed: false, partial: 4_000_000, compatible: true },
  ],
  jobs: [{ id: "phpmyadmin", version: "5.2.3", stage: null, received: 0, total: null, error: "download failed: timeout", done: true }],
};

function renderPage() {
  vi.mocked(api.packagesList).mockResolvedValue({
    packages: [php, pma],
    phpVersion: "8.4.26",
    dbUi: null,
    catalogUpdated: "2026-10-14",
  });
  render(
    <QueryClientProvider client={new QueryClient()}>
      <PackagesPage />
    </QueryClientProvider>,
  );
}

describe("PackagesPage", () => {
  it("installs a version and protects the running one", async () => {
    renderPage();
    expect(await screen.findByText("Running: 8.4.26")).toBeInTheDocument();

    // The installed list comes first; "8.4.26" also appears under Available.
    const installed = screen.getAllByText("8.4.26", { selector: "span" })[0]?.closest("li");
    expect(within(installed as HTMLElement).getByRole("button", { name: "Remove" })).toBeDisabled();

    await userEvent.click(screen.getByRole("button", { name: "Install" }));
    expect(api.packageInstall).toHaveBeenCalledWith("php", "8.5.11");

    const older = screen.getByText("8.3.35", { selector: "span" }).closest("li");
    await userEvent.click(within(older as HTMLElement).getByRole("button", { name: "Use" }));
    expect(api.packageSetActive).toHaveBeenCalledWith("php", "8.3.35");
  });

  it("offers to resume an interrupted download and shows the failure", async () => {
    renderPage();
    await userEvent.click(await screen.findByRole("tab", { name: "phpMyAdmin" }));
    expect(screen.getByText(/Install phpMyAdmin to manage your databases/)).toBeInTheDocument();
    expect(screen.getByText("Not installed: download failed: timeout")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Resume" }));
    expect(api.packageInstall).toHaveBeenCalledWith("phpmyadmin", "5.2.3");
  });
});
