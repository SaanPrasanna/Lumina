import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ConfigWarnings } from "@/components/ConfigWarnings";

describe("ConfigWarnings", () => {
  it("translates known codes with their values and falls back to the message", () => {
    render(
      <ConfigWarnings
        warnings={[
          { code: "php_children_raised", field: "php.fcgi_children", message: "x", values: { from: "2", to: "4" } },
          { code: "something_new", field: "", message: "Raw English message", values: {} },
        ]}
      />,
    );
    expect(screen.getByText("PHP_FCGI_CHILDREN=2 is too low; Lumina uses 4.")).toBeInTheDocument();
    expect(screen.getByText("Raw English message")).toBeInTheDocument();
  });

  it("renders nothing without warnings", () => {
    const { container } = render(<ConfigWarnings warnings={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
