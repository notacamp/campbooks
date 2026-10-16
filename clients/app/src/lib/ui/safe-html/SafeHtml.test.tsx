import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../test/render";

import { estimateHeight, fitZoom, framePolicy, MIN_ZOOM, SafeHtml, SANDBOX } from "./SafeHtml";

function frame(container: HTMLElement): HTMLIFrameElement {
  const element = container.querySelector("iframe");
  if (element === null) {
    throw new Error("no frame");
  }
  return element;
}

describe("SafeHtml", () => {
  it("sandboxes the frame: never scripts, no forms or top navigation, popups only", () => {
    const { container } = renderUi(<SafeHtml html="<p>Hi</p>" title="Message from Maya" />);
    const element = frame(container);
    const tokens = (element.getAttribute("sandbox") ?? "").split(/\s+/u).toSorted();

    // Exactly these: `allow-same-origin` is safe only because scripts never
    // run in the frame, so `allow-scripts` must never join it.
    expect(tokens).toEqual(["allow-popups", "allow-popups-to-escape-sandbox", "allow-same-origin"]);
    expect(SANDBOX).not.toMatch(/allow-scripts|allow-forms|allow-top-navigation|allow-modals/u);
    expect(element).toHaveAttribute("title", "Message from Maya");
    expect(element.getAttribute("srcdoc")).toContain("<p>Hi</p>");
  });

  it("blocks remote images with its own policy until they are asked for", () => {
    const { container, rerender } = renderUi(<SafeHtml html='<img alt="Logo">' title="Message" />);
    expect(frame(container).getAttribute("srcdoc")).toContain(framePolicy(false));
    expect(framePolicy(false)).toContain("img-src data:;");
    expect(framePolicy(false)).toContain("default-src 'none'");
    expect(framePolicy(false)).not.toContain("script-src");

    rerender(<SafeHtml html='<img alt="Logo">' title="Message" remoteImages />);
    expect(frame(container).getAttribute("srcdoc")).toContain("img-src data: https: http:");
  });

  it("sends no referrer and opens links in a new tab", () => {
    const { container } = renderUi(
      <SafeHtml html="<a href='https://example.com'>x</a>" title="M" />,
    );
    const srcdoc = frame(container).getAttribute("srcdoc") ?? "";

    expect(srcdoc).toContain('<meta name="referrer" content="no-referrer">');
    expect(srcdoc).toContain('<base target="_blank">');
  });

  it("never puts the message's CSS in the frame's markup", () => {
    const css = "@media (max-width: 600px) { .c { width: 100% !important } }";
    const { container } = renderUi(<SafeHtml html="<p class='c'>Hi</p>" title="M" css={css} />);
    const srcdoc = frame(container).getAttribute("srcdoc") ?? "";

    expect(srcdoc).not.toContain("@media");
    expect(srcdoc).not.toContain("<style");
  });

  it("starts from an estimate of its height, within bounds", () => {
    const { container } = renderUi(<SafeHtml html="<p>Hi</p>" title="M" />);
    expect(frame(container).style.height).toBe("96px");
    expect(estimateHeight("<p>Hi</p>")).toBe(96);
    expect(estimateHeight(`<p>${"word ".repeat(400)}</p>`)).toBeGreaterThan(400);
    expect(estimateHeight(`<p>${"word ".repeat(40_000)}</p>`)).toBe(1400);
    expect(estimateHeight('<img height="300"><p>x</p>')).toBeGreaterThan(300);
  });

  it("shrinks a layout wider than its frame, down to a floor", () => {
    expect(fitZoom(600, 800)).toBe(1);
    expect(fitZoom(344, 343)).toBe(1);
    expect(fitZoom(600, 343)).toBeCloseTo(0.571, 3);
    expect(fitZoom(2000, 343)).toBe(MIN_ZOOM);
    expect(fitZoom(600, 0)).toBe(1);
  });
});
