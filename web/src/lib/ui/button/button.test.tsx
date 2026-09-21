/**
 * Button smoke test — demonstrates the data-testid-only selector convention.
 *
 * Grammar: `module.feature.element[.part][.recordId]` (dotted camelCase).
 * Tests locate elements ONLY by data-testid — never by role, text, label, or
 * CSS selectors. A copy change in the button label must not break this test.
 */
import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { Button } from "~/lib/ui";

describe("Button", () => {
  it("renders and is findable by testid", () => {
    render(<Button data-testid="ui.button.smoke">Action</Button>);
    expect(screen.getByTestId("ui.button.smoke")).toBeInTheDocument();
  });

  it("renders secondary variant by testid", () => {
    render(
      <Button variant="secondary" data-testid="ui.button.secondary">
        Cancel
      </Button>,
    );
    expect(screen.getByTestId("ui.button.secondary")).toBeInTheDocument();
  });

  it("renders as disabled", () => {
    render(
      <Button disabled data-testid="ui.button.disabled">
        Submit
      </Button>,
    );
    const btn = screen.getByTestId("ui.button.disabled");
    expect(btn).toBeDisabled();
  });
});
