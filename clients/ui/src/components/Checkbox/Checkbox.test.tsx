import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { Checkbox } from "./Checkbox";

describe("Checkbox", () => {
  it("is named by its label and described by its hint, and the label ticks it", () => {
    const onChange = vi.fn();
    renderUi(<Checkbox label="Also sign in" hint="Untick for support@" onChange={onChange} />);

    const box = screen.getByRole("checkbox", { name: "Also sign in" });
    expect(box).toHaveAccessibleDescription("Untick for support@");
    fireEvent.click(screen.getByText("Also sign in"));
    expect(onChange).toHaveBeenCalled();
    expect(box).toBeChecked();
  });
});
