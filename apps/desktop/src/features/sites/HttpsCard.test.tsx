import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { webConflictRequest } from "@/features/services/WebCard";
import type { WebSettings } from "@/features/settings/api";
import * as settingsApi from "@/features/settings/api";
import { HttpsCard } from "@/features/sites/HttpsCard";
import * as ssl from "@/features/sites/ssl";

vi.mock("@/features/sites/ssl", async (orig) => ({
  ...(await orig<typeof ssl>()),
  sslStatus: vi.fn(),
  sslSetEnabled: vi.fn(),
  sslSetForceHttps: vi.fn(),
  sslTrust: vi.fn(),
  sslRegenerate: vi.fn(),
  sslRemoveCa: vi.fn(),
}));
vi.mock("@/features/settings/api", async (orig) => ({
  ...(await orig<typeof settingsApi>()),
  settingsUpdate: vi.fn(),
}));
vi.mock("@/features/services/api", async (orig) => ({
  ...(await orig<typeof import("@/features/services/api")>()),
  serviceStart: vi.fn(),
}));

const ca: ssl.CaStatus = {
  subject: "Lumina Local CA - PC",
  notAfter: 2_000_000_000,
  thumbprint: "AB",
  permitted: ["local", "localhost", "test"],
  certPath: "C:\\Lumina\\ssl\\ca.crt",
  trusted: false,
  coversTld: true,
};

const overview: ssl.SslOverview = {
  enabled: true,
  forceHttps: false,
  httpsPort: 443,
  tld: "test",
  ca,
  sites: [],
  error: null,
  sslDir: "C:\\Lumina\\ssl",
};

function renderCard(data: ssl.SslOverview) {
  vi.mocked(ssl.sslStatus).mockResolvedValue(data);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <HttpsCard />
    </QueryClientProvider>,
  );
}

describe("HttpsCard", () => {
  it("asks to trust an untrusted CA and reports a declined prompt", async () => {
    vi.mocked(ssl.sslTrust).mockResolvedValue("declined");
    renderCard(overview);

    expect(await screen.findByText(/Not trusted yet/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Trust certificate" }));

    expect(ssl.sslTrust).toHaveBeenCalled();
    expect(await screen.findByText("The confirmation was declined, so nothing changed.")).toBeInTheDocument();
  });

  it("switches HTTPS off and on", async () => {
    vi.mocked(ssl.sslSetEnabled).mockResolvedValue(null);
    renderCard({ ...overview, ca: { ...ca, trusted: true } });

    expect(await screen.findByText(/Trusted by this computer/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("switch"));
    expect(ssl.sslSetEnabled).toHaveBeenCalledWith(false);
  });

  it("hides the details while HTTPS is off", async () => {
    renderCard({ ...overview, enabled: false });
    expect(await screen.findByRole("switch")).not.toBeChecked();
    expect(screen.queryByText(/Force HTTPS/)).toBeNull();
  });

  it("removes the CA only after a second confirmation", async () => {
    vi.mocked(ssl.sslRemoveCa).mockResolvedValue("done");
    renderCard({ ...overview, ca: { ...ca, trusted: true } });

    await userEvent.click(await screen.findByRole("button", { name: "Remove certificate authority" }));
    expect(ssl.sslRemoveCa).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Remove and turn HTTPS off" }));
    expect(ssl.sslRemoveCa).toHaveBeenCalled();
  });
});

describe("HTTPS port conflicts", () => {
  const web: WebSettings = { server: "nginx", nginx_version: "", apache_version: "", http_port: 80, https_port: 443 };

  it("moves only the HTTPS port when 443 is taken", async () => {
    const request = webConflictRequest({ port: 443, owner: "skype.exe" }, web);
    await request.apply(8443);
    expect(settingsApi.settingsUpdate).toHaveBeenCalledWith("web", { ...web, https_port: 8443 });
  });

  it("moves HTTP (and HTTPS off 443) when 80 is taken", async () => {
    const request = webConflictRequest({ port: 80, owner: null }, web);
    await request.apply(8080);
    expect(settingsApi.settingsUpdate).toHaveBeenCalledWith("web", { ...web, http_port: 8080, https_port: 8443 });
  });
});
