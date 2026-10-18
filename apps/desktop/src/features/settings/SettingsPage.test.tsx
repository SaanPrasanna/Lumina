import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsPage } from "@/features/settings";
import * as api from "@/features/settings/api";

vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock("@/features/settings/api", async (orig) => ({
  ...(await orig<typeof api>()),
  settingsGet: vi.fn(),
  settingsUpdate: vi.fn(),
}));
vi.mock("@/features/onboarding/api", async (orig) => ({
  ...(await orig<object>()),
  bootstrapStatus: vi.fn().mockResolvedValue({ configured: true, dataRoot: "C:\\Lumina", defaultDataRoot: "C:\\Lumina", language: "en" }),
}));
vi.mock("@/features/services/api", async (orig) => ({ ...(await orig<object>()), servicesStatus: vi.fn().mockResolvedValue(null) }));
vi.mock("@/features/env-path/api", async (orig) => ({ ...(await orig<object>()), editorPresets: vi.fn().mockResolvedValue([]) }));
vi.mock("@/features/updates/api", async (orig) => ({
  ...(await orig<object>()),
  updatesStatus: vi.fn().mockResolvedValue({ current: "0.3.0", channel: "stable", check: true, available: null, checkedAt: null, installing: false, error: null }),
}));

const settings = {
  config: {
    general: { www_root: "", tld: "test", auto_vhosts: true, auto_ssl: true, force_https: true, start_on_login: false, autostart_services: false, close_to_tray: true, lan: false },
    php: { version: "", fcgi_port: 10987, fcgi_fallback_port: 20987, fcgi_children: 10, fcgi_max_requests: 0, xdebug: false },
    web: { server: "nginx", nginx_version: "", apache_version: "", http_port: 80, https_port: 443 },
    database: { engine: "mysql", mysql_version: "", mariadb_version: "", port: 3306 },
    redis: { enabled: false, version: "", port: 6379 },
    postgresql: { enabled: false, version: "", port: 5432 },
    mailpit: { enabled: false, version: "", smtp_port: 1025, ui_port: 8025, php_mail: true },
    editor: { name: "VS Code", command: "code", args: ["{path}"], args_line: ["--goto", "{path}:{line}"] },
    packages: { proxy: "" },
  },
  warnings: [],
  path: "C:\\Lumina\\config.toml",
} as unknown as api.Settings;

function renderPage() {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.settingsGet).mockResolvedValue(settings);
    vi.mocked(api.settingsUpdate).mockResolvedValue(settings);
  });

  it("saves a section only after a change, and can undo it", async () => {
    renderPage();
    const general = within((await screen.findByRole("heading", { name: "General" })).closest("form") as HTMLElement);
    const save = general.getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();

    const autostart = general.getByRole("switch", { name: "Start PHP and the web server when Lumina opens" });
    await userEvent.click(autostart);
    expect(general.getByText("Unsaved changes")).toBeInTheDocument();
    await userEvent.click(general.getByRole("button", { name: "Undo changes" }));
    expect(autostart).not.toBeChecked();
    expect(save).toBeDisabled();

    await userEvent.click(autostart);
    await userEvent.click(save);
    expect(api.settingsUpdate).toHaveBeenCalledWith("general", expect.objectContaining({ autostart_services: true }));
  });

  it("lists every section in the index", async () => {
    renderPage();
    await screen.findByRole("heading", { name: "Mail" });
    const index = screen.getAllByRole("navigation", { name: "Settings sections" })[0];
    expect(within(index).getAllByRole("link")).toHaveLength(10);
  });
});
