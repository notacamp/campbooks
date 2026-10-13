import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { KbdHint } from "./Kbd";

describe("KbdHint", () => {
  it("draws the registry's keys for a shortcut, hidden from screen readers", () => {
    const { container } = renderUi(<KbdHint shortcut="workspace.switch" />);

    expect([...container.querySelectorAll("kbd")].map((kbd) => kbd.textContent)).toEqual([
      "Ctrl",
      "Shift",
      "W",
    ]);
    expect(container.querySelector("[aria-hidden='true']")).not.toBeNull();
  });

  it("renders nothing for a shortcut the registry does not know", () => {
    const { container } = renderUi(<KbdHint shortcut="custom.thing" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("writes then between sequence steps and announces when asked", () => {
    renderUi(<KbdHint shortcut="nav.inbox" withThen announce />);

    expect(screen.getByText("Shortcut: G then I")).toBeInTheDocument();
    expect(screen.getByText("then")).toBeInTheDocument();
  });

  it("speaks the user's language", () => {
    renderUi(<KbdHint shortcut="nav.inbox" announce />, { locale: "pt-PT" });

    expect(screen.getByText("Atalho: G depois I")).toBeInTheDocument();
  });

  it("shows one-off keys", () => {
    const { container } = renderUi(<KbdHint keys="mod+enter" reveal />);

    expect(container).toHaveTextContent("Ctrl↵");
  });
});
