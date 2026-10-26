import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../test/render";

import { StatusText } from "./StatusText";

describe("StatusText", () => {
  it("says the status in words, the dot only repeats it", () => {
    renderUi(<StatusText tone="problem">Needs you to sign in again</StatusText>);

    const text = screen.getByText("Needs you to sign in again");
    expect(text.closest("[data-tone]")).toHaveAttribute("data-tone", "problem");
  });
});
