import { useLayoutEffect, useRef, type RefObject } from "react";

/** Whether this browser has the popover API (the top layer). jsdom does not. */
export function hasTopLayer(): boolean {
  return (
    typeof HTMLElement !== "undefined" &&
    typeof (HTMLElement.prototype as Partial<HTMLElement>).showPopover === "function"
  );
}

export interface TopLayerOptions {
  /** Show the panel (it must not carry `hidden` while open). */
  open: boolean;
  /** Line the panel up with the anchor's start edge, or its end edge. */
  align?: "start" | "end" | undefined;
  /** At least as wide as the anchor (a list under its field). */
  matchWidth?: boolean | undefined;
  /** The anchor scrolled out of view: close the panel. */
  onAway: () => void;
}

/** A space token in px, read where the panel is (0 when it is not set). */
function space(element: Element, name: string): number {
  return Number.parseFloat(getComputedStyle(element).getPropertyValue(name)) || 0;
}

export interface TopLayer<A extends HTMLElement, P extends HTMLElement> {
  /** For what the panel hangs from: a trigger, a field. */
  anchor: RefObject<A | null>;
  /** For the panel, rendered with `popover="manual"` when `onTop`. */
  panel: RefObject<P | null>;
  /** The top layer is used; without the popover API the panel stays in the flow. */
  onTop: boolean;
}

/**
 * Shows an open panel in the top layer, where no ancestor's overflow (a
 * dialog's scrolling body) can clip it, at fixed coordinates next to its
 * anchor: below it, or above when there is more room there; inside the
 * viewport sideways. It follows the anchor on resize and on any scroll, and
 * closes once the anchor has scrolled out of view. Returns the refs to put
 * on the anchor and the panel, and whether the top layer is used.
 */
export function useTopLayer<A extends HTMLElement, P extends HTMLElement>({
  open,
  align = "start",
  matchWidth = false,
  onAway,
}: TopLayerOptions): TopLayer<A, P> {
  const supported = hasTopLayer();
  const anchor = useRef<A>(null);
  const panel = useRef<P>(null);
  const away = useRef(onAway);
  useLayoutEffect(() => {
    away.current = onAway;
  });

  useLayoutEffect(() => {
    const box = panel.current;
    const at = anchor.current;
    if (!supported || !open || box === null || at === null) {
      return;
    }
    const place = () => {
      const rect = at.getBoundingClientRect();
      if (rect.bottom < 0 || rect.top > window.innerHeight) {
        away.current();
        return;
      }
      const gap = space(box, "--space-1");
      const edge = space(box, "--space-2");
      if (matchWidth) {
        box.style.minWidth = `${String(rect.width)}px`;
      }
      const { width, height } = box.getBoundingClientRect();
      const below = window.innerHeight - rect.bottom - gap - edge;
      const above = rect.top - gap - edge;
      const top = height <= below || below >= above ? rect.bottom + gap : rect.top - gap - height;
      const wanted = align === "end" ? rect.right - width : rect.left;
      const left = Math.min(
        Math.max(edge, wanted),
        Math.max(edge, window.innerWidth - width - edge),
      );
      box.style.top = `${String(Math.max(edge, top))}px`;
      box.style.left = `${String(left)}px`;
    };
    box.showPopover();
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      try {
        box.hidePopover();
      } catch {
        // Already hidden, or no longer in the document.
      }
    };
  }, [supported, open, align, matchWidth]);

  return { anchor, panel, onTop: supported };
}
