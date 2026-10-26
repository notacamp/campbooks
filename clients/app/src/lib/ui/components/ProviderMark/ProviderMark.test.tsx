import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { ProviderMark } from "./ProviderMark";

describe("ProviderMark", () => {
  it("draws a provider's initial, hidden from screen readers", () => {
    const { container } = renderUi(<ProviderMark name="zoho Mail" size="lg" />);

    const mark = container.querySelector("[aria-hidden='true']");
    expect(mark).toHaveTextContent("Z");
    expect(mark).toHaveAttribute("data-size", "lg");
  });
});
