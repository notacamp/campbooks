import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { HowDecided } from "./HowDecided";

describe("HowDecided", () => {
  it("keeps the numbers behind a closed disclosure", () => {
    renderUi(
      <HowDecided
        rows={[{ question: "Needs a reply", answer: "Yes.", source: "Rules · 0.90" }]}
        footnote="Footnote"
      />,
    );
    const numbers = screen.getByText("Rules · 0.90");
    expect(numbers).not.toBeVisible();
    fireEvent.click(screen.getByText("How Campbooks decided"));
    expect(numbers).toBeVisible();
    expect(screen.getByText("Footnote")).toBeVisible();
  });

  it("shows nothing without rows", () => {
    const { container } = renderUi(<HowDecided rows={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
