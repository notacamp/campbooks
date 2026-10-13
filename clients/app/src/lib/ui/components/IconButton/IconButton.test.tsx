import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";
import { Visor } from "../Visor/Visor";

import { IconButton } from "./IconButton";

describe("IconButton", () => {
  it("is named by its label and hints its shortcut in the tooltip", () => {
    renderUi(<IconButton label="Next conversation" icon="chevron-down" shortcut="list.next" />);
    const button = screen.getByRole("button", { name: "Next conversation" });

    expect(button).toHaveAttribute("title", "Next conversation (J)");
    expect(button).toHaveAttribute("aria-keyshortcuts", "J");
  });

  it("takes a custom glyph and a touch size", () => {
    renderUi(<IconButton label="Search or ask" icon={<Visor />} size="lg" />);
    const button = screen.getByRole("button", { name: "Search or ask" });

    expect(button).toHaveAttribute("title", "Search or ask");
    expect(button).toHaveAttribute("data-size", "lg");
  });
});
