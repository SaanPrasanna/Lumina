import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ErrorBox } from "@/components/ErrorBox";

describe("ErrorBox", () => {
  it("offers to install what is missing instead of reporting a failure", () => {
    render(
      <ErrorBox
        error={{
          code: "not_installed",
          message: "MySQL is not installed",
          details: { package: "mysql", name: "MySQL", version: "" },
        }}
      />,
    );
    expect(screen.getByText("MySQL is not installed yet.")).toBeInTheDocument();
    expect(screen.queryByText("The service could not be started or stopped.")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Install MySQL" })).toHaveAttribute("href", "#/packages?package=mysql");
  });

  it("names the missing version", () => {
    render(<ErrorBox error={{ code: "not_installed", message: "", details: { package: "php", name: "PHP", version: "8.2" } }} />);
    expect(screen.getByText("PHP 8.2 is not installed yet.")).toBeInTheDocument();
  });
});
