import { act, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Clamp } from "./Clamp";

/** A ResizeObserver that reports at once, with `scrollHeight` set for the content. */
function measuring(scrollHeight: number) {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      callback: () => void;
      constructor(callback: () => void) {
        this.callback = callback;
      }
      observe() {
        this.callback();
      }
      disconnect() {
        // Nothing to let go of.
      }
    },
  );
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(scrollHeight);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(100);
}

describe("Clamp", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("opens to show it all and folds back, saying which it is", () => {
    renderUi(
      <Clamp moreLabel="Show the whole email" lessLabel="Show less">
        <p>Hi Sam,</p>
      </Clamp>,
    );
    const more = screen.getByRole("button", { name: "Show the whole email" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    expect(more).toHaveAttribute("aria-controls");
    fireEvent.click(more);
    const less = screen.getByRole("button", { name: "Show less" });
    expect(less).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Hi Sam,")).toBeVisible();
    fireEvent.click(less);
    expect(screen.getByRole("button", { name: "Show the whole email" })).toBeVisible();
  });

  it("offers no button when everything already shows", () => {
    measuring(80);
    renderUi(
      <Clamp moreLabel="Show the whole email" lessLabel="Show less">
        <p>Thursday works.</p>
      </Clamp>,
    );
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("opens when focus moves into the hidden part", () => {
    measuring(400);
    renderUi(
      <Clamp moreLabel="Show the whole email" lessLabel="Show less">
        <p>Long letter</p>
        <a href="#later">A link far down</a>
      </Clamp>,
    );
    const link = screen.getByRole("link", { name: "A link far down" });
    act(() => {
      link.focus();
    });
    expect(screen.getByRole("button", { name: "Show less" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });
});
