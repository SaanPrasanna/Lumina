import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QuickAppPage } from "@/features/quickapp";
import * as api from "@/features/quickapp/api";

type Handler = (event: { payload: unknown }) => void;
const handlers: Record<string, Handler> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    handlers[name] = handler;
    return Promise.resolve(() => {});
  }),
}));
vi.mock("@/features/quickapp/api", async (orig) => ({
  ...(await orig<typeof api>()),
  quickappOverview: vi.fn(),
  quickappCreate: vi.fn(),
  quickappCancel: vi.fn(),
  quickappDeletePartial: vi.fn(),
}));

const laravel: api.Recipe = {
  id: "laravel",
  name: "Laravel",
  description: "The PHP framework for web artisans, created with the official Laravel installer.",
  homepage: "https://laravel.com",
  category: "framework",
  needs: ["php", "composer", "database"],
  options: [
    {
      id: "kit",
      label: "Starter kit",
      default: "none",
      choices: [
        { value: "none", label: "None", needs: [] },
        { value: "react", label: "React", needs: ["node"] },
      ],
    },
    { id: "testing", label: "Testing framework", default: "pest", choices: [{ value: "pest", label: "Pest", needs: [] }] },
  ],
};

const job: api.Job = {
  id: 7,
  recipe: "laravel",
  recipeName: "Laravel",
  name: "shop",
  dir: "C:\\Lumina\\www\\shop",
  domain: "shop.test",
  url: "https://shop.test",
  state: "running",
  step: { index: 3, total: 6, label: "Create the Laravel application" },
  download: null,
  error: null,
  warning: null,
  lines: ["> composer global require laravel/installer"],
  dropped: 0,
  partial: false,
  createdDatabase: "shop",
  logFile: "C:\\Lumina\\logs\\quickapp\\shop.log",
};

function renderPage(currentJob: api.Job | null = null) {
  vi.mocked(api.quickappOverview).mockResolvedValue({
    recipes: [laravel],
    www: "C:\\Lumina\\www",
    tld: "test",
    missing: ["node"],
    taken: ["laravel-app", "blog"],
    job: currentJob,
    database: { engine: "mysql", host: "127.0.0.1", port: 3307, user: "root", password: "" },
  });
  render(
    <QueryClientProvider client={new QueryClient()}>
      <QuickAppPage />
    </QueryClientProvider>,
  );
}

describe("QuickAppPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("creates a project with the chosen options", async () => {
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: /Laravel/ }));
    const name = screen.getByRole("textbox", { name: "Project name" });
    // "laravel-app" is taken, so the next free name is suggested.
    expect(name).toHaveValue("laravel-app-2");
    expect(screen.queryByText(/Lumina installs it first/)).not.toBeInTheDocument();

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Starter kit" }), "react");
    expect(screen.getByText(/Lumina installs it first: Node.js/)).toBeInTheDocument();

    await userEvent.clear(name);
    await userEvent.type(name, "My Shop");
    expect(name).toHaveValue("my-shop");
    expect(screen.getByText("my-shop.test")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Create project" }));
    expect(api.quickappCreate).toHaveBeenCalledWith("laravel", "my-shop", { kit: "react", testing: "pest" });
  });

  it("refuses a taken name", async () => {
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: /Laravel/ }));
    const name = screen.getByRole("textbox", { name: "Project name" });
    await userEvent.clear(name);
    await userEvent.type(name, "blog");
    expect(screen.getByText("A folder with this name already exists.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create project" })).toBeDisabled();
  });

  it("streams output and cancels", async () => {
    renderPage(job);
    expect(await screen.findByText("Step 3 of 6: Create the Laravel application")).toBeInTheDocument();
    act(() =>
      handlers["quickapp-output"]?.({
        payload: { jobId: 7, first: 1, lines: ["Creating a \"laravel/laravel\" project"], step: null, download: null },
      }),
    );
    expect(screen.getByLabelText("Output")).toHaveTextContent('Creating a "laravel/laravel" project');
    // Output of another job is ignored.
    act(() => handlers["quickapp-output"]?.({ payload: { jobId: 8, first: 0, lines: ["other"], step: null, download: null } }));
    expect(screen.getByLabelText("Output")).not.toHaveTextContent("other");

    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(api.quickappCancel).toHaveBeenCalled();
  });

  it("deletes a partial project after confirming", async () => {
    renderPage({ ...job, state: "failed", error: "composer failed (exit code 1)", partial: true });
    expect(await screen.findByText("shop could not be created")).toBeInTheDocument();
    expect(screen.getByText("composer failed (exit code 1)")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Delete partial project" }));
    expect(api.quickappDeletePartial).not.toHaveBeenCalled();
    expect(screen.getByText(/and the database shop\?/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(api.quickappDeletePartial).toHaveBeenCalled();
  });
});
