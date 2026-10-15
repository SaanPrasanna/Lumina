import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "@/features/updates/api";
import { UpdateBanner, UpdatesCard } from "@/features/updates/UpdatesCard";

vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock("@/features/updates/api", async (orig) => ({
  ...(await orig<typeof api>()),
  updatesStatus: vi.fn(),
  updatesCheck: vi.fn(),
  updatesConfigure: vi.fn(),
  updatesInstall: vi.fn(),
}));

const base: api.UpdateStatus = {
  current: "0.9.0",
  channel: "stable",
  check: true,
  available: null,
  checkedAt: 1_760_000_000,
  installing: false,
  error: null,
};

function wrap(ui: React.ReactNode) {
  render(<QueryClientProvider client={new QueryClient()}>{ui}</QueryClientProvider>);
}

describe("UpdatesCard", () => {
  beforeEach(() => vi.clearAllMocks());

  it("installs an update only after confirming", async () => {
    vi.mocked(api.updatesStatus).mockResolvedValue({
      ...base,
      available: { version: "1.0.0", notes: "Signed installers.", date: null },
    });
    wrap(<UpdatesCard />);
    expect(await screen.findByText("Lumina 1.0.0 is available.")).toBeInTheDocument();
    expect(screen.getByText("Signed installers.")).toBeInTheDocument();
    expect(screen.getByText("Lumina 0.9.0")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Install and restart…" }));
    expect(api.updatesInstall).not.toHaveBeenCalled();
    expect(screen.getByText(/The services that are running now start again afterwards/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Install now" }));
    expect(api.updatesInstall).toHaveBeenCalled();
  });

  it("switches the channel and checks on demand", async () => {
    vi.mocked(api.updatesStatus).mockResolvedValue(base);
    vi.mocked(api.updatesConfigure).mockResolvedValue({ ...base, channel: "beta" });
    vi.mocked(api.updatesCheck).mockResolvedValue({ ...base, error: "could not fetch a valid release JSON" });
    wrap(<UpdatesCard />);
    expect(await screen.findByText(/Lumina is up to date/)).toBeInTheDocument();

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Update channel" }), "beta");
    expect(api.updatesConfigure).toHaveBeenCalledWith("beta", true);

    await userEvent.click(screen.getByRole("button", { name: "Check now" }));
    expect(await screen.findByText(/The update check failed: could not fetch a valid release JSON/)).toBeInTheDocument();
  });
});

describe("UpdateBanner", () => {
  it("shows a waiting update until dismissed", async () => {
    vi.mocked(api.updatesStatus).mockResolvedValue({
      ...base,
      available: { version: "1.0.0", notes: null, date: null },
    });
    const open = vi.fn();
    wrap(<UpdateBanner onOpen={open} />);
    await userEvent.click(await screen.findByRole("button", { name: "View" }));
    expect(open).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Later" }));
    expect(screen.queryByText("Lumina 1.0.0 is available.")).not.toBeInTheDocument();
  });
});
