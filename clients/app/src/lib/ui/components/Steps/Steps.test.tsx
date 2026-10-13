import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Steps } from "./Steps";

describe("Steps", () => {
  it("is an ordered list, named, one item per step", () => {
    renderUi(<Steps label="What happens next" steps={["First", "Second"]} />);
    const list = screen.getByRole("list", { name: "What happens next" });

    expect(list.tagName).toBe("OL");
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((item) => item.textContent),
    ).toEqual(["1First", "2Second"]);
  });
});
