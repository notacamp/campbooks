import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../test/render";

import { Panel } from "./Panel";

describe("Panel", () => {
  it("is a labelled region holding its children", () => {
    renderUi(<Panel label="A conflict">{"Notion's version"}</Panel>);
    expect(screen.getByRole("region", { name: "A conflict" })).toHaveTextContent(
      "Notion's version",
    );
  });

  it("says which look and layout it has", () => {
    renderUi(
      <Panel label="Kept" variant="dashed" row className="extra">
        x
      </Panel>,
    );
    const panel = screen.getByRole("region", { name: "Kept" });
    expect(panel).toHaveAttribute("data-variant", "dashed");
    expect(panel).toHaveAttribute("data-row", "true");
    expect(panel).toHaveClass("extra");
  });
});
