import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";
import { ProfilesPage } from "@/features/profiles";
import * as api from "@/features/profiles/api";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("@/features/profiles/api", async (orig) => ({
  ...(await orig<typeof api>()),
  profilesList: vi.fn(),
  profileSwitch: vi.fn(),
  profileSaveCurrent: vi.fn(),
  profileDelete: vi.fn(),
}));

const legacy: api.ProfileView = {
  name: "Legacy",
  web: "nginx",
  web_version: "",
  php: "8.3",
  database: { engine: "mysql", version: "" },
  redis: false,
  postgresql: false,
  current: true,
  missing: [],
};
const modern: api.ProfileView = { ...legacy, name: "Modern", web: "apache", php: "8.4", current: false };
const old: api.ProfileView = { ...legacy, name: "Old", php: "7.4", current: false, missing: ["PHP 7.4"] };

function renderPage() {
  vi.mocked(api.profilesList).mockResolvedValue({
    profiles: [legacy, modern, old],
    stack: legacy,
    active: "Legacy",
  });
  render(
    <QueryClientProvider client={new QueryClient()}>
      <MemoryRouter>
        <ProfilesPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const card = (name: string) => screen.getByText(name, { selector: "span" }).closest("li") as HTMLElement;

describe("ProfilesPage", () => {
  it("lists profiles with their stacks and switches", async () => {
    vi.mocked(api.profileSwitch).mockResolvedValue({ stop: [], start: [], keep: [] });
    renderPage();
    expect(await screen.findByText("Modern", { selector: "span" })).toBeInTheDocument();
    expect(within(card("Modern")).getByText("Apache newest · PHP 8.4 · MySQL newest")).toBeInTheDocument();
    expect(within(card("Legacy")).getByText("Current")).toBeInTheDocument();
    expect(within(card("Legacy")).queryByRole("button", { name: "Switch" })).toBeNull();

    await userEvent.click(within(card("Modern")).getByRole("button", { name: "Switch" }));
    expect(api.profileSwitch).toHaveBeenCalledWith("Modern");
  });

  it("blocks profiles that need something not installed", async () => {
    renderPage();
    await screen.findByText("Old", { selector: "span" });
    expect(within(card("Old")).getByRole("button", { name: "Switch" })).toBeDisabled();
    expect(within(card("Old")).getByText(/Not installed: PHP 7.4/)).toBeInTheDocument();
  });

  it("saves the current stack and deletes only after confirming", async () => {
    renderPage();
    await userEvent.type(await screen.findByPlaceholderText("e.g. Legacy"), "Work");
    await userEvent.click(screen.getByRole("button", { name: "Save as profile" }));
    expect(api.profileSaveCurrent).toHaveBeenCalledWith("Work");

    await userEvent.click(within(card("Modern")).getByRole("button", { name: "Delete" }));
    expect(api.profileDelete).not.toHaveBeenCalled();
    await userEvent.click(within(card("Modern")).getByRole("button", { name: "Delete for good" }));
    expect(api.profileDelete).toHaveBeenCalledWith("Modern");
  });
});
