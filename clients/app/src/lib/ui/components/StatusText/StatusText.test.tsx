import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";
import { ProviderMark } from "../ProviderMark/ProviderMark";

import { StatusText } from "./StatusText";

describe("StatusText and ProviderMark", () => {
  it("says the status in words, the dot only repeats it", () => {
    renderUi(<StatusText tone="problem">Needs you to sign in again</StatusText>);

    const text = screen.getByText("Needs you to sign in again");
    expect(text.closest("[data-tone]")).toHaveAttribute("data-tone", "problem");
  });

  it("draws a provider's initial, hidden from screen readers", () => {
    const { container } = renderUi(<ProviderMark name="zoho Mail" size="lg" />);

    const mark = container.querySelector("[aria-hidden='true']");
    expect(mark).toHaveTextContent("Z");
    expect(mark).toHaveAttribute("data-size", "lg");
  });
});
