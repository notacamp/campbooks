import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Visor, VISOR_STATES } from "./Visor";

const GLYPH_STATES = ["happy", "dizzy", "starry", "hearts"] as const;

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

  it.each(GLYPH_STATES)("draws two hidden inline SVG eyes for %s", (state) => {
    const { container } = renderUi(<Visor state={state} />);
    const eyes = container.querySelectorAll("svg");

    expect(eyes).toHaveLength(2);
    for (const eye of eyes) expect(eye).toHaveAttribute("aria-hidden", "true");
    expect(container.querySelectorAll("i")).toHaveLength(0);
  });

  it("keeps pill eyes for the other states", () => {
    const { container } = renderUi(<Visor state="thinking" />);

    expect(container.querySelectorAll("i")).toHaveLength(2);
    expect(container.querySelectorAll("svg")).toHaveLength(0);
  });

  it("is untinted unless asked", () => {
    const plain = renderUi(<Visor state="hearts" />);
    expect(plain.container.firstElementChild).toHaveAttribute("data-tinted", "false");

    const tinted = renderUi(<Visor state="hearts" tinted />);
    expect(tinted.container.firstElementChild).toHaveAttribute("data-tinted", "true");
  });
});
