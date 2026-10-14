import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as api from "@/features/services/api";
import { PhpCard } from "@/features/services/PhpCard";
import type { Settings } from "@/features/settings/api";

vi.mock("@/features/services/api", async (orig) => ({
  ...(await orig<typeof api>()),
  phpStatus: vi.fn(),
  phpVersions: vi.fn(),
  phpStart: vi.fn(),
  phpStop: vi.fn(),
  phpRestart: vi.fn(),
  phpSetVersion: vi.fn(),
  phpOpenLog: vi.fn(),
}));

const settings: Settings = {
  config: {
    schema_version: 1,
    php: { version: "", fcgi_port: 10987, fcgi_fallback_port: 20987, fcgi_children: 10, fcgi_max_requests: 0 },
    editor: { name: "VS Code", command: "code", args: ["{path}"] },
  },
  warnings: [],
  source: "file",
  path: "C:/Lumina/config.toml",
};
vi.mock("@/features/settings/api", async (orig) => ({
  ...(await orig<object>()),
  settingsGet: () => Promise.resolve(settings),
}));

const installs: api.PhpInstall[] = [
  { version: "8.4.26", dir: "a", cgi: "a" },
  { version: "8.3.35", dir: "b", cgi: "b" },
];

function renderCard() {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <PhpCard />
    </QueryClientProvider>,
  );
}

describe("PhpCard", () => {
  it("shows a running pool on the fallback port", async () => {
    vi.mocked(api.phpVersions).mockResolvedValue(installs);
    vi.mocked(api.phpStatus).mockResolvedValue({
      status: "running",
      runtime: { version: "8.4.26", port: 20987, fellBack: true, children: 10, maxRequests: 0, ini: "x" },
      pid: 42,
      processes: 11,
      restarts: 0,
      error: null,
      logFile: "x",
    });
    renderCard();

    expect(await screen.findByText("Running")).toBeInTheDocument();
    expect(screen.getByText("fallback")).toBeInTheDocument();
    expect(screen.getByText("Port 10987 was busy, so PHP uses 20987.")).toBeInTheDocument();
    expect(screen.getByText("11")).toBeInTheDocument();
    expect(screen.getByText("Unlimited")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Newest installed (8.4.26)" })).toBeInTheDocument();
  });

  it("starts PHP when stopped", async () => {
    vi.mocked(api.phpVersions).mockResolvedValue(installs);
    vi.mocked(api.phpStatus).mockResolvedValue({
      status: "stopped",
      runtime: null,
      pid: null,
      processes: 0,
      restarts: 0,
      error: null,
      logFile: "x",
    });
    vi.mocked(api.phpStart).mockImplementation(() => new Promise(() => {}));
    renderCard();

    await userEvent.click(await screen.findByRole("button", { name: "Start" }));
    expect(api.phpStart).toHaveBeenCalled();
  });
});
