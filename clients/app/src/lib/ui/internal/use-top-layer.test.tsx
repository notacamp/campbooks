import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useTopLayer } from "./use-top-layer";

function Harness({ open, onAway }: { open: boolean; onAway: () => void }) {
  const {
    anchor,
    panel,
    onTop: top,
  } = useTopLayer<HTMLButtonElement, HTMLDivElement>({
    open,
    onAway,
  });
  return (
    <div>
      <button ref={anchor} type="button">
        Due
      </button>
      <div ref={panel} data-testid="panel" data-top={top ? "" : undefined}>
        Grid
      </div>
    </div>
  );
}

/** A box at `top`/`left`, `width` by `height` (what getBoundingClientRect answers). */
const box = (top: number, left: number, width: number, height: number) =>
  ({
    top,
    left,
    width,
    height,
    bottom: top + height,
    right: left + width,
    x: left,
    y: top,
    toJSON: () => ({}),
  }) as DOMRect;

/** Where the anchor and the panel are; the window is jsdom's 1024 × 768. */
function layout(anchor: DOMRect, panel: DOMRect) {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    return this.tagName === "BUTTON" ? anchor : panel;
  });
}

const PROTOTYPE = HTMLElement.prototype as unknown as Record<string, unknown>;

describe("useTopLayer", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    delete PROTOTYPE["showPopover"];
    delete PROTOTYPE["hidePopover"];
  });

  it("leaves the panel in the flow where there is no popover API", () => {
    render(<Harness open onAway={vi.fn()} />);
    expect(screen.getByTestId("panel")).not.toHaveAttribute("data-top");
    expect(screen.getByTestId("panel").style.top).toBe("");
  });

  describe("with the popover API", () => {
    const show = vi.fn();
    const hide = vi.fn();
    beforeEach(() => {
      show.mockClear();
      hide.mockClear();
      PROTOTYPE["showPopover"] = show;
      PROTOTYPE["hidePopover"] = hide;
    });

    it("shows the panel in the top layer, under its anchor", () => {
      layout(box(100, 50, 200, 32), box(0, 0, 300, 200));
      render(<Harness open onAway={vi.fn()} />);
      const panel = screen.getByTestId("panel");
      expect(panel).toHaveAttribute("data-top");
      expect(show).toHaveBeenCalledOnce();
      expect(panel.style.top).toBe("132px");
      expect(panel.style.left).toBe("50px");
    });

    it("opens above when there is no room below, and stays inside the window", () => {
      layout(box(700, 900, 120, 32), box(0, 0, 300, 200));
      render(<Harness open onAway={vi.fn()} />);
      const panel = screen.getByTestId("panel");
      expect(panel.style.top).toBe("500px");
      expect(panel.style.left).toBe("724px");
    });

    it("closes once the anchor scrolls out of view, and hides when closed", () => {
      const onAway = vi.fn();
      layout(box(100, 50, 200, 32), box(0, 0, 300, 200));
      const { rerender } = render(<Harness open onAway={onAway} />);
      vi.restoreAllMocks();
      PROTOTYPE["showPopover"] = show;
      PROTOTYPE["hidePopover"] = hide;
      layout(box(-200, 50, 200, 32), box(0, 0, 300, 200));

      globalThis.dispatchEvent(new Event("scroll"));
      expect(onAway).toHaveBeenCalledOnce();

      rerender(<Harness open={false} onAway={onAway} />);
      expect(hide).toHaveBeenCalledOnce();
    });
  });
});
