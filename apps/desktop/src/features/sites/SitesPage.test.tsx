import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as settingsApi from "@/features/settings/api";
import { HostsStatus, SitesPage } from "@/features/sites";
import * as api from "@/features/sites/api";

vi.mock("@/features/sites/api", async (orig) => ({
  ...(await orig<typeof api>()),
  sitesList: vi.fn(),
  sitesSync: vi.fn(),
  siteOpen: vi.fn(),
  siteOpenFolder: vi.fn(),
}));
vi.mock("@/features/sites/ssl", async (orig) => ({
  ...(await orig<typeof import("@/features/sites/ssl")>()),
  sslStatus: vi.fn().mockResolvedValue({
    enabled: false,
    forceHttps: false,
    httpsPort: 443,
    tld: "test",
    ca: null,
    sites: [],
    error: null,
    sslDir: "x",
  }),
}));
vi.mock("@/features/settings/api", async (orig) => ({
  ...(await orig<typeof settingsApi>()),
  settingsGet: vi.fn(),
  settingsUpdate: vi.fn(),
}));

const hosts: api.HostsReport = {
  file: "C:\\Windows\\System32\\drivers\\etc\\hosts",
  inSync: true,
  missing: [],
  extra: [],
  pending: false,
  last: null,
  error: null,
};

const overview: api.SitesOverview = {
  sites: [
    {
      name: "blog",
      domain: "blog.test",
      path: "C:\\Lumina\\www\\blog",
      docroot: "C:\\Lumina\\www\\blog\\public",
      docrootSource: { kind: "detected", subdir: "public" },
      docrootExists: true,
    },
  ],
  skipped: [{ name: "old", reason: { kind: "disabled" } }],
  wwwRoot: "C:\\Lumina\\www",
  tld: "test",
  autoVhosts: true,
  webRunning: true,
  httpPort: 80,
  httpsPort: null,
  hosts,
};

function renderPage() {
  vi.mocked(api.sitesList).mockResolvedValue(overview);
  vi.mocked(settingsApi.settingsGet).mockResolvedValue({
    config: {
      general: { tld: "test", auto_vhosts: true },
      sites: { old: { enabled: false, docroot: "" } },
    },
    warnings: [],
    source: "file",
    path: "x",
  } as unknown as settingsApi.Settings);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <SitesPage />
    </QueryClientProvider>,
  );
}

describe("SitesPage", () => {
  it("lists sites with their docroot and opens them", async () => {
    renderPage();
    expect(await screen.findByText("blog.test")).toBeInTheDocument();
    expect(screen.getByText("Folder blog · serves public/")).toBeInTheDocument();
    expect(screen.getByText("The hosts file has every site.")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(api.siteOpen).toHaveBeenCalledWith("blog.test");
  });

  it("turns a disabled folder back on by dropping its override", async () => {
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Turn on" }));
    await vi.waitFor(() => expect(settingsApi.settingsUpdate).toHaveBeenCalledWith("sites", {}));
  });
});

describe("HostsStatus", () => {
  it("offers a retry after a declined prompt", async () => {
    const onRetry = vi.fn();
    render(
      <HostsStatus
        hosts={{ ...hosts, inSync: false, missing: ["a.test", "b.test"], last: { outcome: "declined", message: null } }}
        webRunning
        busy={false}
        onRetry={onRetry}
      />,
    );
    expect(
      screen.getByText("The administrator prompt was declined, so 2 addresses are not in the hosts file yet."),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Update hosts file" }));
    expect(onRetry).toHaveBeenCalled();
  });

  it("shows the open prompt without a retry button", () => {
    render(<HostsStatus hosts={{ ...hosts, inSync: false, pending: true }} webRunning busy={false} onRetry={vi.fn()} />);
    expect(screen.getByText("Waiting for you to approve the administrator prompt…")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
