import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as api from "@/features/services/api";
import { alternativePort } from "@/features/services/PortConflictDialog";
import { WebCard } from "@/features/services/WebCard";
import type { WebSettings } from "@/features/settings/api";
import { portConflictOf } from "@/lib/ipc";

vi.mock("@/features/services/api", async (orig) => ({
  ...(await orig<typeof api>()),
  webVersions: vi.fn(),
  serviceStart: vi.fn(),
  serviceReload: vi.fn(),
  serviceRestart: vi.fn(),
  serviceStop: vi.fn(),
}));

const config: WebSettings = {
  server: "nginx",
  nginx_version: "",
  apache_version: "",
  http_port: 80,
  https_port: 443,
};

const stopped: api.WebStatus = {
  server: "nginx",
  status: "stopped",
  runtime: null,
  pid: null,
  processes: 0,
  error: null,
  logFile: "x",
};

function renderCard(status: api.WebStatus, onPortConflict = vi.fn()) {
  vi.mocked(api.webVersions).mockResolvedValue([{ version: "1.30.5", dir: "a", exe: "a", homebrew: null }]);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <WebCard status={status} config={config} onPortConflict={onPortConflict} />
    </QueryClientProvider>,
  );
  return onPortConflict;
}

describe("WebCard", () => {
  it("opens the port-conflict flow when port 80 is taken", async () => {
    vi.mocked(api.serviceStart).mockImplementation(() =>
      Promise.reject({
        code: "port_in_use",
        message: "port 80 is already in use by System (pid 4)",
        details: { port: 80, owner: "System (pid 4)" },
      }),
    );
    const onPortConflict = renderCard(stopped);

    await userEvent.click(await screen.findByRole("button", { name: "Start" }));

    await vi.waitFor(() =>
      expect(onPortConflict).toHaveBeenCalledWith(
        expect.objectContaining({ conflict: { port: 80, owner: "System (pid 4)" }, isWeb: true, service: "nginx" }),
      ),
    );
    expect(api.serviceStart).toHaveBeenCalledWith("nginx");
  });

  it("reports a reload that needed no signal", async () => {
    vi.mocked(api.serviceReload).mockResolvedValue({ kind: "unchanged" });
    renderCard({
      ...stopped,
      status: "running",
      runtime: { server: "nginx", version: "1.30.5", httpPort: 80, httpsPort: null, phpPort: 10987 },
      processes: 2,
    });

    await userEvent.click(screen.getByRole("button", { name: "Reload" }));

    expect(await screen.findByText("Nothing changed; no reload was needed.")).toBeInTheDocument();
    expect(api.serviceReload).toHaveBeenCalledWith("nginx");
  });
});

describe("port conflicts", () => {
  it("suggests 8080 for 80, 8443 for 443, else the next port", () => {
    expect(alternativePort(80)).toBe(8080);
    expect(alternativePort(443)).toBe(8443);
    expect(alternativePort(8080)).toBe(8081);
    expect(alternativePort(3306)).toBe(3307);
  });

  it("only treats port_in_use for the web port as a conflict", () => {
    const err = { code: "port_in_use", message: "", details: { port: 80, owner: null } };
    expect(portConflictOf(err, 80)).toEqual({ port: 80, owner: null });
    expect(portConflictOf(err, 8080)).toBeNull();
    expect(portConflictOf({ code: "service", message: "" }, 80)).toBeNull();
  });
});
