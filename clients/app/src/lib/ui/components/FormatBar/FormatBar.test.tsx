import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { FormatBar } from "./FormatBar";

describe("FormatBar", () => {
  it("presses what it shows without taking the editor's focus", () => {
    const onBold = vi.fn();
    renderUi(
      <FormatBar
        label="Format"
        always
        items={[
          { id: "mention", label: "Mention", glyph: "@", onPress: vi.fn() },
          { id: "bold", label: "Bold", icon: "bold", pressed: true, onPress: onBold },
        ]}
      />,
    );
    const bold = screen.getByRole("button", { name: "Bold" });
    expect(bold).toHaveAttribute("aria-pressed", "true");
    expect(fireEvent.mouseDown(bold)).toBe(false);
    fireEvent.click(bold);
    expect(onBold).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("toolbar", { name: "Format" })).toBeInTheDocument();
  });
});
