import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Visor, VISOR_STATES } from "./Visor";

describe("Visor", () => {
  it("is decorative by default", () => {
    const { container } = renderUi(<Visor state="thinking" size="xl" />);

    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
    expect(container.firstElementChild).toHaveAttribute("data-state", "thinking");
  });

  it.each(VISOR_STATES)("announces the %s state when asked", (state) => {
    renderUi(<Visor state={state} announce />);

    expect(screen.getByRole("img")).toHaveAccessibleName(/Campbooks/);
  });
});
