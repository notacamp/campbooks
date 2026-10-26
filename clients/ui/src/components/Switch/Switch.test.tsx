import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { Switch } from "./Switch";

describe("Switch", () => {
  it("is a switch named for what it turns on, in the state it is in", () => {
    renderUi(<Switch label="Show Work" checked onChange={vi.fn()} />);

    expect(screen.getByRole("switch", { name: "Show Work" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("asks for the opposite state when pressed", () => {
    const onChange = vi.fn();
    renderUi(<Switch label="Show Work" checked={false} onChange={onChange} />);

    fireEvent.click(screen.getByRole("switch", { name: "Show Work" }));

    expect(onChange).toHaveBeenCalledWith(true);
  });
});
