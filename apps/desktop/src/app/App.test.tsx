import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "@/app/App";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock("@/features/onboarding/api", () => ({
  BOOTSTRAP_QUERY_KEY: ["bootstrap"],
  bootstrapStatus: () =>
    Promise.resolve({ configured: true, dataRoot: "C:/Lumina", defaultDataRoot: "C:/Lumina", language: "si" }),
  setLanguage: vi.fn(),
}));

describe("App", () => {
  afterEach(() => i18n.changeLanguage("en"));

  it("renders the whole UI in the saved language from the first paint", async () => {
    // StrictMode as in main.tsx: its double mount is what exposed the bug.
    render(
      <StrictMode>
        <QueryClientProvider client={new QueryClient()}>
          <App />
        </QueryClientProvider>
      </StrictMode>,
    );

    // Sidebar (mounted at start-up) and the language picker agree on Sinhala.
    expect(await screen.findByRole("link", { name: "සැකසීම්" })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Settings" })).not.toBeInTheDocument();
    expect(screen.getByRole("combobox")).toHaveValue("si");
  });
});
