// Finds UI copy that does not fit where it is shown: text cut off by its box
// (an ellipsis, a clamp, `overflow: hidden`), text past the edge of the
// screen, a placeholder wider than its field, and a page that scrolls
// sideways. Only our copy counts: the "longest" pseudo-locale wraps every
// string in FIT_MARK (`.storybook/longest.ts`), so a long demo name that
// truncates on purpose is never reported.
import { FIT_MARK } from "../../.storybook/longest";

export interface Misfit {
  /** The copy, without the marks. */
  text: string;
  problem:
    "cut off" | "spills out" | "off screen" | "placeholder cut off" | "page scrolls sideways";
  /** Where: the element that cuts it, as `tag.class`. */
  at: string;
  /** Width (or height) the text needs, and what it gets, in px. */
  needs: number;
  has: number;
}

const SLACK = 1;

const unmarked = (text: string) => text.replaceAll(FIT_MARK, "").replaceAll(/\s+/gu, " ").trim();

/** `tag.class`, with the CSS module hash dropped (`_label_1x2y3_4` → `label`). */
function nameOf(element: Element): string {
  const name = [...element.classList]
    .map((value) => /^_?([A-Za-z][\w-]*?)_[\w-]{5}_\d+$/u.exec(value)?.[1] ?? value)
    .slice(0, 2)
    .join(".");
  return name === "" ? element.localName : `${element.localName}.${name}`;
}

const clips = (value: string) => value === "hidden" || value === "clip";
const scrolls = (value: string) => value === "auto" || value === "scroll";

/** Visually hidden (screen reader only) or collapsed to nothing. */
const collapsed = (element: Element) => element.clientWidth <= 1 || element.clientHeight <= 1;

/** The box a text node is laid out in: its nearest ancestor that is not `display: inline`. */
function layoutBox(node: Text): HTMLElement | null {
  let element = node.parentElement;
  while (element !== null && getComputedStyle(element).display === "inline") {
    element = element.parentElement;
  }
  return element;
}

function textRect(node: Text): DOMRect {
  const range = document.createRange();
  range.selectNodeContents(node);
  return range.getBoundingClientRect();
}

function checkText(node: Text, found: Misfit[]): void {
  const parent = node.parentElement;
  const text = unmarked(node.data);
  if (parent === null || text === "" || !parent.checkVisibility({ visibilityProperty: true })) {
    return;
  }
  const rect = textRect(node);
  if (rect.width === 0 && rect.height === 0) {
    return;
  }
  // Glyphs may overhang a tight line box (`line-height: 1`) by a few px; a
  // clamp or a cut line is at least half a line, so that is the bar for height.
  const ySlack = Math.max(SLACK, Number.parseFloat(getComputedStyle(parent).fontSize) / 2);

  // The box the text sits in, cut short (an ellipsis or a clamp measures as overflow).
  // Only when the box holds this copy alone, so other content in it is never blamed.
  const box = layoutBox(node);
  if (box !== null && !collapsed(box) && unmarked(box.textContent) === text) {
    const style = getComputedStyle(box);
    if (clips(style.overflowX) && box.scrollWidth - box.clientWidth > SLACK) {
      found.push({
        text,
        problem: "cut off",
        at: nameOf(box),
        needs: box.scrollWidth,
        has: box.clientWidth,
      });
      return;
    }
    if (clips(style.overflowY) && box.scrollHeight - box.clientHeight > ySlack) {
      found.push({
        text,
        problem: "cut off",
        at: nameOf(box),
        needs: box.scrollHeight,
        has: box.clientHeight,
      });
      return;
    }
  }

  // Any ancestor that clips it, up to the first one that scrolls (scrolling
  // is fine: a row of chips that scrolls sideways shows every chip in turn).
  let scrolled = false;
  for (let element: Element | null = parent; element !== null; element = element.parentElement) {
    const style = getComputedStyle(element);
    // An inline element (a link, a <strong>) has no box of its own to clip
    // with, and measures 0×0: look past it.
    if (style.display === "inline") {
      continue;
    }
    if (scrolls(style.overflowX) || scrolls(style.overflowY)) {
      scrolled = element !== document.documentElement && element !== document.body;
      break;
    }
    if (collapsed(element)) {
      return;
    }
    const outer = element.getBoundingClientRect();
    const left = outer.left + element.clientLeft;
    const top = outer.top + element.clientTop;
    const right = left + element.clientWidth;
    const bottom = top + element.clientHeight;
    if (clips(style.overflowX) && (rect.left < left - SLACK || rect.right > right + SLACK)) {
      found.push({
        text,
        problem: "cut off",
        at: nameOf(element),
        needs: Math.ceil(rect.width),
        has: element.clientWidth,
      });
      return;
    }
    if (clips(style.overflowY) && (rect.top < top - ySlack || rect.bottom > bottom + ySlack)) {
      found.push({
        text,
        problem: "cut off",
        at: nameOf(element),
        needs: Math.ceil(rect.height),
        has: element.clientHeight,
      });
      return;
    }
  }

  // Copy that runs out of a box that does not clip it (a label wrapping out
  // of a fixed-height button), up to the box that positions it: past an
  // absolutely positioned element (a tooltip, a badge) or a scroller,
  // landing outside is on purpose.
  for (let element: Element | null = box; element !== null; element = element.parentElement) {
    const style = getComputedStyle(element);
    if (scrolls(style.overflowX) || scrolls(style.overflowY)) {
      break;
    }
    if (style.display !== "inline" && style.display !== "contents" && !collapsed(element)) {
      const outer = element.getBoundingClientRect();
      const outside =
        rect.left < outer.left - SLACK ||
        rect.right > outer.right + SLACK ||
        rect.top < outer.top - ySlack ||
        rect.bottom > outer.bottom + ySlack;
      if (outside) {
        const tall = rect.bottom > outer.bottom + ySlack || rect.top < outer.top - ySlack;
        found.push({
          text,
          problem: "spills out",
          at: nameOf(element),
          needs: Math.ceil(tall ? rect.height : rect.width),
          has: Math.floor(tall ? outer.height : outer.width),
        });
        return;
      }
    }
    if (style.position === "absolute" || style.position === "fixed") {
      break;
    }
  }

  if (
    !scrolled &&
    (rect.left < -SLACK || rect.right > document.documentElement.clientWidth + SLACK)
  ) {
    found.push({
      text,
      problem: "off screen",
      at: nameOf(box ?? parent),
      needs: Math.ceil(rect.right),
      has: document.documentElement.clientWidth,
    });
  }
}

const measure = document.createElement("canvas").getContext("2d");

function checkPlaceholder(field: HTMLInputElement | HTMLTextAreaElement, found: Misfit[]): void {
  const text = unmarked(field.placeholder);
  if (measure === null || field.value !== "" || collapsed(field) || !field.checkVisibility()) {
    return;
  }
  const style = getComputedStyle(field);
  if (field instanceof HTMLTextAreaElement) {
    // A textarea wraps its placeholder; only a single line can be cut short.
    return;
  }
  measure.font = style.font;
  const needs = Math.ceil(measure.measureText(field.placeholder.replaceAll(FIT_MARK, "")).width);
  const has =
    field.clientWidth -
    Number.parseFloat(style.paddingLeft) -
    Number.parseFloat(style.paddingRight);
  if (needs - has > SLACK) {
    found.push({
      text,
      problem: "placeholder cut off",
      at: nameOf(field),
      needs,
      has: Math.floor(has),
    });
  }
}

/** Every misfit of our copy in the document as it is laid out now. */
export function misfits(root: Element = document.body): Misfit[] {
  const found: Misfit[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) =>
      node.nodeValue?.includes(FIT_MARK) === true &&
      (node.parentElement?.closest("script, style, template, title, option") ?? null) === null
        ? NodeFilter.FILTER_ACCEPT
        : NodeFilter.FILTER_REJECT,
  });
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    checkText(node as Text, found);
  }
  for (const field of root.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>(
    "input[placeholder], textarea[placeholder]",
  )) {
    if (field.placeholder.includes(FIT_MARK)) {
      checkPlaceholder(field, found);
    }
  }
  const page = document.documentElement;
  if (page.scrollWidth - page.clientWidth > SLACK) {
    found.push({
      text: "",
      problem: "page scrolls sideways",
      at: "html",
      needs: page.scrollWidth,
      has: page.clientWidth,
    });
  }
  return found;
}

export function formatMisfit({ text, problem, at, needs, has }: Misfit): string {
  const where = `${at} (needs ${String(needs)}px, has ${String(has)}px)`;
  return text === "" ? `${problem}: ${where}` : `${problem}: "${text}" in ${where}`;
}

/**
 * Waits until the story has settled: fonts loaded, finite animations done
 * (spinners run forever and are not waited for) and no DOM change for a
 * moment (screens fetch their data from the mocked API first).
 */
export async function settle({ quietMs = 150, maxMs = 5000 } = {}): Promise<void> {
  const start = performance.now();
  let last = start;
  const observer = new MutationObserver(() => {
    last = performance.now();
  });
  observer.observe(document.body, {
    subtree: true,
    childList: true,
    characterData: true,
    attributes: true,
  });
  try {
    await document.fonts.ready;
    while (performance.now() - last < quietMs && performance.now() - start < maxMs) {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    // A cancelled animation rejects `finished`; it is done all the same.
    await Promise.allSettled(
      document
        .getAnimations()
        .filter((animation) => animation.effect?.getComputedTiming().iterations !== Infinity)
        .map((animation) => animation.finished),
    );
  } finally {
    observer.disconnect();
  }
}
