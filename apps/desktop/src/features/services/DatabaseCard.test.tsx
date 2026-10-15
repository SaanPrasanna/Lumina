import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import * as api from "@/features/services/api";
import { DatabaseCard } from "@/features/services/DatabaseCard";
import type { DatabaseSettings } from "@/features/settings/api";

vi.mock("@/features/services/api", async (orig) => ({
  ...(await orig<typeof api>()),
  dbVersions: vi.fn(),
  serviceStart: vi.fn(),
  serviceStop: vi.fn(),
  serviceRestart: vi.fn(),
}));

const config: DatabaseSettings = { engine: "mysql", mysql_version: "", mariadb_version: "", port: 3306 };
const stopped: api.DbStatus = { engine: "mysql", status: "stopped", runtime: null, pid: null, error: null, logFile: "x" };

function renderCard(status: api.DbStatus, onPortConflict = vi.fn()) {
  vi.mocked(api.dbVersions).mockResolvedValue([{ version: "8.4.11", dir: "a", exe: "a", homebrew: null }]);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <DatabaseCard status={status} config={config} onPortConflict={onPortConflict} />
    </QueryClientProvider>,
  );
  return onPortConflict;
}

describe("DatabaseCard", () => {
  it("shows the running server, its data folder and how to log in", async () => {
    renderCard({
      ...stopped,
      status: "running",
      runtime: { engine: "mysql", version: "8.4.11", port: 3306, dataDir: "C:/Lumina/data/mysql-8.4" },
    });
    expect(screen.getByText("MySQL 8.4.11")).toBeInTheDocument();
    expect(screen.getByText("C:/Lumina/data/mysql-8.4")).toBeInTheDocument();
    expect(screen.getByText(/root with no password/)).toBeInTheDocument();
  });

  it("offers another port when 3306 is taken by an existing MySQL", async () => {
    vi.mocked(api.serviceStart).mockImplementation(() =>
      Promise.reject({ code: "port_in_use", message: "", details: { port: 3306, owner: "mysqld.exe (pid 9)" } }),
    );
    const onPortConflict = renderCard(stopped);

    await userEvent.click(await screen.findByRole("button", { name: "Start" }));

    await vi.waitFor(() =>
      expect(onPortConflict).toHaveBeenCalledWith(
        expect.objectContaining({ conflict: { port: 3306, owner: "mysqld.exe (pid 9)" }, service: "MySQL", isWeb: false }),
      ),
    );
    expect(api.serviceStart).toHaveBeenCalledWith("mysql");
  });
});
