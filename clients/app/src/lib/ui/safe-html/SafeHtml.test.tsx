import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../test/render";

import { estimateHeight, framePolicy, SafeHtml } from "./SafeHtml";

function frame(container: HTMLElement): HTMLIFrameElement {
  const element = container.querySelector("iframe");
  if (element === null) {
    throw new Error("no frame");
  }
  return element;
}

describe("SafeHtml", () => {
  it("sandboxes the frame: no scripts, no same origin, popups only", () => {
    const { container } = renderUi(<SafeHtml html="<p>Hi</p>" title="Message from Maya" />);
    const element = frame(container);

    expect(element.getAttribute("sandbox")).toBe("allow-popups allow-popups-to-escape-sandbox");
    expect(element.getAttribute("sandbox")).not.toMatch(/allow-scripts|allow-same-origin/u);
    expect(element).toHaveAttribute("title", "Message from Maya");
    expect(element.getAttribute("srcdoc")).toContain("<p>Hi</p>");
  });

  it("blocks remote images with its own policy until they are asked for", () => {
    const { container, rerender } = renderUi(<SafeHtml html='<img alt="Logo">' title="Message" />);
    expect(frame(container).getAttribute("srcdoc")).toContain(framePolicy(false));
    expect(framePolicy(false)).toContain("img-src data:;");
    expect(framePolicy(false)).toContain("default-src 'none'");

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

  it("sizes the frame from its content, within bounds", () => {
    expect(estimateHeight("<p>Hi</p>")).toBe(96);
    expect(estimateHeight(`<p>${"word ".repeat(400)}</p>`)).toBeGreaterThan(400);
    expect(estimateHeight(`<p>${"word ".repeat(40_000)}</p>`)).toBe(1400);
    expect(estimateHeight('<img height="300"><p>x</p>')).toBeGreaterThan(300);
  });
});
