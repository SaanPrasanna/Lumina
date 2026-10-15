import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as packagesApi from "@/features/packages/api";
import * as servicesApi from "@/features/services/api";
import * as settingsApi from "@/features/settings/api";
import { ShareButton, SharePanel, TunnelPage, useSiteShare } from "@/features/tunnel";
import * as api from "@/features/tunnel/api";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@/features/tunnel/api", async (orig) => ({
  ...(await orig<typeof api>()),
  tunnelOverview: vi.fn(),
  tunnelStart: vi.fn(),
  tunnelStop: vi.fn(),
  tunnelSetToken: vi.fn(),
  tunnelSetDomain: vi.fn(),
}));
vi.mock("@/features/services/api", async (orig) => ({
  ...(await orig<typeof servicesApi>()),
  serviceStart: vi.fn(),
}));
vi.mock("@/features/settings/api", async (orig) => ({
  ...(await orig<typeof settingsApi>()),
  settingsUpdate: vi.fn(),
}));
vi.mock("@/features/packages/api", async (orig) => ({
  ...(await orig<typeof packagesApi>()),
  packagesList: vi.fn().mockResolvedValue({ packages: [], phpVersion: null, dbUi: null, catalogUpdated: "" }),
  packageInstall: vi.fn(),
}));

const tunnel: api.TunnelView = {
  name: "blog",
  publicUrl: "https://my-app.ngrok-free.app",
  host: "blog.test",
  port: 80,
  startedAt: 1_760_000_000,
  qrSvg: '<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"></svg>',
};

const ready: api.TunnelOverview = {
  installed: "3.39.11",
  available: "3.39.11",
  hasToken: true,
  tokenError: null,
  domain: "my-app.ngrok-free.app",
  tunnels: [],
  stopped: null,
  logFile: "C:\\Lumina\\logs\\ngrok.log",
};

function wrap(ui: React.ReactNode) {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>,
  );
}

const web = { server: "nginx", http_port: 80, https_port: 443 } as settingsApi.WebSettings;

function Row({ overview }: { overview: api.TunnelOverview }) {
  const share = useSiteShare("blog", overview, web);
  return (
    <>
      <ShareButton share={share} />
      <SharePanel share={share} />
    </>
  );
}

describe("Share", () => {
  beforeEach(() => vi.clearAllMocks());

  it("opens a tunnel with the reserved domain and shows its address", async () => {
    vi.mocked(api.tunnelStart).mockResolvedValue(tunnel);
    const { rerender } = wrap(<Row overview={ready} />);
    await userEvent.click(screen.getByRole("button", { name: "Share" }));
    expect(api.tunnelStart).toHaveBeenCalledWith("blog", "my-app.ngrok-free.app");

    // The page refetches the overview, which now lists the tunnel.
    rerender(
      <QueryClientProvider client={new QueryClient()}>
        <MemoryRouter>
          <Row overview={{ ...ready, tunnels: [tunnel] }} />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    expect(await screen.findByText("https://my-app.ngrok-free.app")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "QR code for https://my-app.ngrok-free.app" })).toBeInTheDocument();

    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    await userEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith("https://my-app.ngrok-free.app");
    expect(await screen.findByRole("button", { name: "Copied" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Stop sharing" }));
    expect(api.tunnelStop).toHaveBeenCalledWith("blog");
  });

  it("points to the Sharing page when the account is not set up", async () => {
    vi.mocked(api.tunnelStart).mockRejectedValue({ code: "tunnel_no_token", message: "no ngrok authtoken is saved" });
    wrap(<Row overview={{ ...ready, hasToken: false, domain: "" }} />);
    await userEvent.click(screen.getByRole("button", { name: "Share" }));
    expect(api.tunnelStart).toHaveBeenCalledWith("blog", null);
    expect(await screen.findByText(/Sharing needs your ngrok authtoken/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Set up sharing" })).toHaveAttribute("href", "/tunnel");
  });
});

describe("Share with a busy port", () => {
  beforeEach(() => vi.clearAllMocks());

  it("offers another port and shares again", async () => {
    vi.mocked(api.tunnelStart)
      .mockRejectedValueOnce({
        code: "port_in_use",
        message: "port 80 is already in use by httpd.exe (pid 23536)",
        details: { port: 80, owner: "httpd.exe (pid 23536)" },
      })
      .mockResolvedValueOnce(tunnel);
    wrap(<Row overview={ready} />);
    await userEvent.click(screen.getByRole("button", { name: "Share" }));
    // jsdom does not open <dialog>s, so the dialog's contents count as hidden.
    const use = await screen.findByRole("button", { name: "Use port 8080", hidden: true });
    expect(screen.getByText(/httpd.exe \(pid 23536\) is already using port 80/)).toBeInTheDocument();
    await userEvent.click(use);
    expect(settingsApi.settingsUpdate).toHaveBeenCalledWith("web", { ...web, http_port: 8080, https_port: 8443 });
    expect(servicesApi.serviceStart).toHaveBeenCalled();
    await vi.waitFor(() => expect(api.tunnelStart).toHaveBeenCalledTimes(2));
  });
});

describe("TunnelPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("installs ngrok and saves the authtoken", async () => {
    vi.mocked(api.tunnelOverview).mockResolvedValue({ ...ready, installed: null, hasToken: false, domain: "" });
    wrap(<TunnelPage />);
    await userEvent.click(await screen.findByRole("button", { name: "Install ngrok 3.39.11" }));
    expect(packagesApi.packageInstall).toHaveBeenCalledWith("ngrok", "3.39.11");

    const token = screen.getByLabelText("Authtoken");
    expect(token).toHaveAttribute("type", "password");
    await userEvent.type(token, "2abcDEFghiJKLmnoPQRstu_3vwxYZ");
    const account = screen.getByRole("region", { name: "ngrok account" });
    await userEvent.click(within(account).getByRole("button", { name: "Save" }));
    expect(api.tunnelSetToken).toHaveBeenCalledWith("2abcDEFghiJKLmnoPQRstu_3vwxYZ");
  });

  it("lists shared sites and why sharing stopped", async () => {
    vi.mocked(api.tunnelOverview).mockResolvedValue({ ...ready, tunnels: [tunnel], stopped: "ngrok refused the authtoken" });
    wrap(<TunnelPage />);
    expect(await screen.findByText("https://my-app.ngrok-free.app")).toBeInTheDocument();
    expect(screen.getByText("ngrok 3.39.11 is installed.")).toBeInTheDocument();
    expect(screen.getByText("Your authtoken is saved in the system keychain.")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("ngrok refused the authtoken");
  });
});
