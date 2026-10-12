import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";
import { Chip, ChipRow } from "../Chip/Chip";

import { Segmented } from "./Segmented";

describe("Segmented and Chip", () => {
  it("marks the chosen segment pressed and reports changes", () => {
    const onChange = vi.fn();
    renderUi(
      <Segmented
        label="Show"
        value="all"
        onChange={onChange}
        options={[
          { value: "all", label: "All" },
          { value: "unread", label: "Unread" },
        ]}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Unread" }));

    expect(screen.getByRole("group", { name: "Show" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "All" })).toHaveAttribute("aria-pressed", "true");
    expect(onChange).toHaveBeenCalledWith("unread");
  });

  it("renders toggle chips in a named row", () => {
    renderUi(
      <ChipRow label="Streams">
        <Chip pressed>Primary</Chip>
        <Chip>Updates</Chip>
      </ChipRow>,
    );

    expect(screen.getByRole("button", { name: "Primary" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Updates" })).not.toHaveAttribute("aria-pressed");
  });
});
