import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../test/render";

import { Clamp } from "./Clamp";

describe("Clamp", () => {
  it("opens to show it all and folds back, saying which it is", () => {
    renderUi(
      <Clamp moreLabel="Show the whole email" lessLabel="Show less">
        <p>Hi Sam,</p>
      </Clamp>,
    );
    const more = screen.getByRole("button", { name: "Show the whole email" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(more);
    const less = screen.getByRole("button", { name: "Show less" });
    expect(less).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Hi Sam,")).toBeVisible();
    fireEvent.click(less);
    expect(screen.getByRole("button", { name: "Show the whole email" })).toBeVisible();
  });
});
