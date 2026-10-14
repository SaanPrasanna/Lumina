import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FirstRunPage } from "@/features/onboarding/FirstRunPage";
import * as api from "@/features/onboarding/api";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@/features/onboarding/api", () => ({
  BOOTSTRAP_QUERY_KEY: ["bootstrap"],
  bootstrapComplete: vi.fn(),
  setLanguage: vi.fn(),
}));

const status: api.BootstrapStatus = {
  configured: false,
  dataRoot: null,
  defaultDataRoot: "/home/dev/Lumina",
  language: "en",
};

function renderPage() {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <FirstRunPage status={status} />
    </QueryClientProvider>,
  );
}

describe("FirstRunPage", () => {
  beforeEach(() => {
    vi.mocked(api.bootstrapComplete).mockReset();
  });

  it("prefills the default data root and submits the edited value", async () => {
    vi.mocked(api.bootstrapComplete).mockResolvedValue({ ...status, configured: true });
    renderPage();

    const input = screen.getByLabelText("Data folder");
    expect(input).toHaveValue("/home/dev/Lumina");

    await userEvent.clear(input);
    await userEvent.type(input, "/srv/lumina");
    await userEvent.click(screen.getByRole("button", { name: "Get started" }));

    expect(api.bootstrapComplete).toHaveBeenCalledWith("/srv/lumina", "en");
  });

  it("shows a translated error from the backend", async () => {
    // A lazily rejected promise, like a real failed `invoke`.
    vi.mocked(api.bootstrapComplete).mockImplementation(() =>
      Promise.reject({ code: "io", message: "Access is denied." }),
    );
    renderPage();

    await userEvent.click(screen.getByRole("button", { name: "Get started" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("A file or folder could not be accessed.");
    expect(alert).toHaveTextContent("Access is denied.");
  });
});
