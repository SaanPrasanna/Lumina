import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as api from "@/features/services/api";
import { PhpCard } from "@/features/services/PhpCard";
import type { PhpSettings } from "@/features/settings/api";

vi.mock("@/features/services/api", async (orig) => ({
  ...(await orig<typeof api>()),
  phpVersions: vi.fn(),
  serviceStart: vi.fn(),
  serviceStop: vi.fn(),
  serviceRestart: vi.fn(),
  serviceOpenLog: vi.fn(),
  phpSetVersion: vi.fn(),
}));

const config: PhpSettings = {
  version: "",
  fcgi_port: 10987,
  fcgi_fallback_port: 20987,
  fcgi_children: 10,
  fcgi_max_requests: 0,
};

const installs: api.Install[] = [
  { version: "8.4.26", dir: "a", exe: "a", homebrew: null },
  { version: "8.3.35", dir: "b", exe: "b", homebrew: null },
];

const stopped: api.PhpStatus = {
  status: "stopped",
  runtime: null,
  pid: null,
  processes: 0,
  restarts: 0,
  error: null,
  logFile: "x",
};

function renderCard(status: api.PhpStatus) {
  vi.mocked(api.phpVersions).mockResolvedValue(installs);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <PhpCard status={status} config={config} />
    </QueryClientProvider>,
  );
}

describe("PhpCard", () => {
  it("shows a running pool on the fallback port", async () => {
    renderCard({
      ...stopped,
      status: "running",
      runtime: { version: "8.4.26", port: 20987, fellBack: true, children: 10, maxRequests: 0, ini: "x" },
      pid: 42,
      processes: 11,
    });

    expect(screen.getByText("Running")).toBeInTheDocument();
    expect(screen.getByText("fallback")).toBeInTheDocument();
    expect(screen.getByText("Port 10987 was busy, so PHP uses 20987.")).toBeInTheDocument();
    expect(screen.getByText("11")).toBeInTheDocument();
    expect(screen.getByText("Unlimited")).toBeInTheDocument();
    expect(await screen.findByRole("option", { name: "Newest installed (8.4.26)" })).toBeInTheDocument();
  });

  it("starts PHP when stopped", async () => {
    vi.mocked(api.serviceStart).mockImplementation(() => new Promise(() => {}));
    renderCard(stopped);

    await userEvent.click(await screen.findByRole("button", { name: "Start" }));
    expect(api.serviceStart).toHaveBeenCalledWith("php-fcgi");
  });
});
