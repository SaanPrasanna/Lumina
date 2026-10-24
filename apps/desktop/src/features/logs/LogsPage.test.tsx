import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { LogsPage } from "@/features/logs";
import * as api from "@/features/logs/api";

vi.mock("@/features/logs/api", async (orig) => ({
  ...(await orig<typeof api>()),
  logsList: vi.fn(),
  logsRead: vi.fn(),
  logsOpen: vi.fn(),
  logsOpenFolder: vi.fn(),
}));

const sources: api.LogSource[] = [
  { id: "app", group: "app", path: "C:\\Users\\me\\AppData\\Local\\dev.lumina.app\\logs\\lumina.log", size: 10, modified: 0 },
  { id: "nginx_error.log", group: "service", path: "C:\\Lumina\\logs\\nginx_error.log", size: 10, modified: 0 },
  { id: "quickapp/shop.log", group: "quickapp", path: "C:\\Lumina\\logs\\quickapp\\shop.log", size: 10, modified: 0 },
];

function renderPage() {
  vi.mocked(api.logsList).mockResolvedValue(sources);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <LogsPage />
    </QueryClientProvider>,
  );
}

const EMPTY: api.Chunk = { text: "", offset: 0, size: 0, reset: false, skipped: false };

describe("LogsPage", () => {
  // jsdom has no layout; the virtualized list needs a size to show rows.
  // Newer virtual-core reads the scroll element's client size, older its offset size.
  beforeAll(() => {
    for (const [prop, size] of [
      ["offsetHeight", 480],
      ["offsetWidth", 800],
      ["clientHeight", 480],
      ["clientWidth", 800],
    ] as const) {
      Object.defineProperty(HTMLElement.prototype, prop, { configurable: true, get: () => size });
    }
  });
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.logsRead).mockResolvedValue(EMPTY);
  });

  it("tails the chosen log and filters it", async () => {
    vi.mocked(api.logsRead).mockResolvedValueOnce({
      text: "[INFO] started nginx\n[WARN] slow request\n2026/10/15 [error] connect() failed\npartial",
      offset: 80,
      size: 80,
      reset: false,
      skipped: true,
    });
    renderPage();
    const log = await screen.findByRole("log", { name: "Log lines" });
    expect(await screen.findByText("[INFO] started nginx")).toBeInTheDocument();
    // The cut-off last line waits for its end.
    expect(log).not.toHaveTextContent("partial");
    expect(screen.getByText(/Showing the end of a long log/)).toBeInTheDocument();
    expect(api.logsRead).toHaveBeenCalledWith("app", null);
    expect(screen.getByRole("combobox", { name: "Log" })).toHaveValue("app");

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Show" }), "error");
    expect(log).toHaveTextContent("connect() failed");
    expect(log).not.toHaveTextContent("slow request");

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Show" }), "all");
    await userEvent.type(screen.getByRole("searchbox", { name: "Filter" }), "SLOW");
    expect(log).toHaveTextContent("slow request");
    expect(log).not.toHaveTextContent("started nginx");
    expect(screen.getByText("1 of 3 lines")).toBeInTheDocument();
  });

  it("switches logs and opens the file", async () => {
    renderPage();
    const select = await screen.findByRole("combobox", { name: "Log" });
    await userEvent.selectOptions(select, "nginx_error.log");
    await waitFor(() => expect(api.logsRead).toHaveBeenCalledWith("nginx_error.log", null));
    expect(screen.getByText("Nothing written yet. New lines appear here as they are written.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Open file" }));
    expect(api.logsOpen).toHaveBeenCalledWith("nginx_error.log");
  });
});

describe("log helpers", () => {
  it("joins lines split across reads", () => {
    const lines: string[] = [];
    let pending = api.appendText(lines, "", "one\ntw");
    pending = api.appendText(lines, pending, "o\r\nthree\n");
    expect(lines).toEqual(["one", "two", "three"]);
    expect(pending).toBe("");
  });

  it("reads severities the way servers write them", () => {
    expect(api.levelOf('{"lvl":"eror","msg":"failed to reconnect session"}')).toBe("error");
    expect(api.levelOf("PHP Fatal error:  Uncaught Error")).toBe("error");
    expect(api.levelOf("[Wed Oct 15] [core:warn] AH00098")).toBe("warn");
    expect(api.levelOf("started `nginx` (pid 1)")).toBe("info");
  });
});
