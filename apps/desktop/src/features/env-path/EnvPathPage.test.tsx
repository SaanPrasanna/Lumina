import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { EnvPathPage } from "@/features/env-path";
import * as api from "@/features/env-path/api";

vi.mock("@/features/env-path/api", async (orig) => ({
  ...(await orig<typeof api>()),
  pathStatus: vi.fn(),
  pathAdd: vi.fn(),
  pathRemove: vi.fn(),
  terminalOpen: vi.fn(),
}));

const base: api.PathOverview = {
  shimDir: "C:\\Lumina\\bin\\.shims",
  added: false,
  addedSystem: false,
  files: [],
  shadowed: [],
  shims: [
    { name: "php", program: "C:\\Lumina\\bin\\php\\8.4.26\\php.exe", args: [], describes: "PHP 8.4.26" },
    { name: "composer", program: "C:\\Lumina\\bin\\php\\8.4.26\\php.exe", args: [], describes: "Composer 2.10.3 on PHP 8.4.26" },
  ],
  nativeShims: true,
};

function renderPage(data: api.PathOverview) {
  vi.mocked(api.pathStatus).mockResolvedValue(data);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <EnvPathPage />
    </QueryClientProvider>,
  );
}

describe("EnvPathPage", () => {
  it("adds Lumina to PATH and lists what each command runs", async () => {
    vi.mocked(api.pathAdd).mockResolvedValue({ ...base, added: true });
    renderPage(base);
    expect(await screen.findByText("Lumina is not on your PATH")).toBeInTheDocument();
    expect(screen.getByText("Composer 2.10.3 on PHP 8.4.26")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Add Lumina to PATH" }));
    expect(api.pathAdd).toHaveBeenCalled();
    expect(await screen.findByText("Lumina is on your PATH")).toBeInTheDocument();
    expect(screen.getByText(/Open a new terminal/)).toBeInTheDocument();
  });

  it("warns when another PHP comes first and offers removal", async () => {
    renderPage({ ...base, added: true, shadowed: [{ command: "php", found: "C:\\php\\php.exe" }] });
    expect(await screen.findByText(/finds `php` at C:\\php\\php.exe first/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Remove from PATH" }));
    expect(api.pathRemove).toHaveBeenCalled();
  });
});
