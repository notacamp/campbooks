import { useEffect, useRef, useState } from "react";

import styles from "./SafeHtml.module.css";

/**
 * What the frame's own policy allows. The page's policy still applies on
 * top (a `srcdoc` frame inherits it), so this can only narrow it: no
 * scripts, no fetches, no fonts from anywhere, no forms, and images only
 * from `data:` until the member asks for remote ones.
 */
export function framePolicy(remoteImages: boolean): string {
  return [
    "default-src 'none'",
    `img-src data:${remoteImages ? " https: http:" : ""}`,
    "style-src 'unsafe-inline'",
    "font-src data:",
    "form-action 'none'",
    "base-uri 'none'",
  ].join("; ");
}

/**
 * The frame's sandbox. Never `allow-scripts`: nothing in the frame ever
 * runs, so it can share the app's origin (`allow-same-origin`) for the one
 * thing the app needs from it, reading the message's size from outside to
 * fit it. No forms, no top navigation; links open a new tab without an
 * opener.
 */
export const SANDBOX = "allow-same-origin allow-popups allow-popups-to-escape-sandbox";

/**
 * The frame's base look, as an attribute: the page's policy blocks style
 * sheets in the frame. Mail is written for a white page, so it gets one in
 * every theme, on a rounded card.
 */
const BODY_STYLE = [
  "margin: 0",
  "padding: 12px 16px",
  "font: 15px/1.55 -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif",
  "color: #1f2328",
  "background: #ffffff",
  "overflow-wrap: break-word",
  "color-scheme: light",
].join("; ");

/**
 * The root's size is the frame's measure, so the message cannot set it: an
 * inline `!important` beats any rule, the message's `@media` rules included.
 */
const ROOT_STYLE = "height: auto !important; min-height: 0 !important; max-height: none !important";

/** The document the frame shows. */
function frameDocument(html: string, remoteImages: boolean): string {
  return [
    `<!doctype html><html style="${ROOT_STYLE}"><head>`,
    '<meta charset="utf-8">',
    `<meta http-equiv="Content-Security-Policy" content="${framePolicy(remoteImages)}">`,
    '<meta name="referrer" content="no-referrer">',
    '<base target="_blank">',
    `</head><body style="${BODY_STYLE}">`,
    html,
    "</body></html>",
  ].join("");
}

/** Smallest and largest estimated heights, in px. */
const MIN_HEIGHT = 96;
const MAX_HEIGHT = 1400;

/**
 * A first height for the frame from the markup alone, used until the
 * message has been measured (or if it cannot be): its text and declared
 * image heights.
 */
export function estimateHeight(html: string): number {
  const text = html.replaceAll(/<[^>]*>/gu, " ").replaceAll(/\s+/gu, " ");
  const blocks = (html.match(/<(?:p|br|li|tr|h[1-6]|div)\b/giu) ?? []).length;
  const images = [...html.matchAll(/<img\b[^>]*\bheight="(\d+)"/giu)].reduce(
    (sum, match) => sum + Math.min(Number(match[1]), 400),
    0,
  );
  const lines = Math.ceil(text.length / 70) + blocks;
  return Math.round(Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, 24 + lines * 23 + images)));
}

/** The smallest scale a wide message is shown at; past it, it scrolls sideways in its frame. */
export const MIN_ZOOM = 0.5;

/** The tallest a measured frame grows, in px. */
export const MAX_MEASURED_HEIGHT = 30_000;

/**
 * How many times a frame is resized for one document: enough for late images
 * and window resizes, and an end to any layout that answers its own resize.
 */
export const MAX_RESIZES = 60;

/**
 * The scale that fits a message `contentWidth` wide into a frame
 * `frameWidth` wide, as mobile mail apps shrink a fixed-width layout:
 * 1 when it fits, never below {@link MIN_ZOOM}.
 */
export function fitZoom(contentWidth: number, frameWidth: number): number {
  if (frameWidth <= 0 || contentWidth <= frameWidth + 1) {
    return 1;
  }
  return Math.max(MIN_ZOOM, Math.floor((frameWidth / contentWidth) * 1000) / 1000);
}

/**
 * Apply the message's `@media` rules (filtered by the server) to the
 * frame's document, and to nothing else: a constructed style sheet, not a
 * `<style>` element, which the page's policy blocks in the frame. Loads
 * the rules could still name are under the frame's policy all the same.
 */
function adoptStyles(doc: Document, css: string | undefined): void {
  const view = doc.defaultView;
  if (view === null) {
    return;
  }
  if (css === undefined || css === "") {
    doc.adoptedStyleSheets = [];
    return;
  }
  const sheet = new view.CSSStyleSheet();
  sheet.replaceSync(css);
  doc.adoptedStyleSheets = [sheet];
}

/**
 * Fit the message to the frame's width (shrinking a layout that is too
 * wide) and return the height it then takes.
 */
function fit(doc: Document, frameWidth: number): number {
  const root = doc.documentElement;
  doc.body.style.removeProperty("zoom");
  const zoom = fitZoom(root.scrollWidth, frameWidth);
  if (zoom < 1) {
    doc.body.style.setProperty("zoom", String(zoom));
  }
  // Wider than even the smallest scale: it scrolls sideways, so leave room
  // for the scrollbar.
  const view = doc.defaultView;
  const scrollbar =
    view !== null && root.scrollWidth > root.clientWidth ? view.innerHeight - root.clientHeight : 0;
  const height = Math.ceil(root.getBoundingClientRect().height) + Math.max(0, scrollbar);
  return Math.min(MAX_MEASURED_HEIGHT, height);
}

export interface SafeHtmlProps {
  /** HTML the server sanitised. Still untrusted: shown only in the sandbox. */
  html: string;
  /** Accessible name of the frame ("Message from Maya Okafor"). */
  title: string;
  /** Let remote images load (the member chose "Show images"). */
  remoteImages?: boolean | undefined;
  /** The message's `@media` rules, filtered by the server; frame only. */
  css?: string | null | undefined;
}

/**
 * Untrusted HTML (an email body) in a sandboxed frame: no scripts, its own
 * strict policy, links that can only open a new tab without an opener, as
 * tall as its content and fitted to its width. The only place the app
 * renders HTML it did not write (AGENTS.md).
 */
export function SafeHtml({ html, title, remoteImages = false, css }: SafeHtmlProps) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [measured, setMeasured] = useState<number | undefined>();
  const srcDoc = frameDocument(html, remoteImages);
  const rules = css ?? undefined;

  useEffect(() => {
    const element = frame.current;
    if (element === null) {
      return;
    }
    let observer: ResizeObserver | undefined;
    let resizes = 0;
    let last: number | undefined;
    const settle = () => {
      const doc = element.contentDocument;
      if (doc?.URL !== "about:srcdoc") {
        return;
      }
      const height = fit(doc, element.clientWidth);
      if (last !== undefined && Math.abs(last - height) < 1) {
        return;
      }
      resizes += 1;
      if (resizes > MAX_RESIZES) {
        observer?.disconnect();
        return;
      }
      last = height;
      setMeasured(height);
    };
    const attach = () => {
      const doc = element.contentDocument;
      // Not this message's document yet, or not readable: keep the estimate.
      if (doc?.URL !== "about:srcdoc") {
        return;
      }
      adoptStyles(doc, rules);
      resizes = 0;
      last = undefined;
      observer?.disconnect();
      observer = new ResizeObserver(settle);
      observer.observe(element);
      observer.observe(doc.documentElement);
      settle();
    };
    element.addEventListener("load", attach);
    if (element.contentDocument?.readyState === "complete") {
      attach();
    }
    return () => {
      element.removeEventListener("load", attach);
      observer?.disconnect();
    };
  }, [srcDoc, rules]);

  return (
    <iframe
      ref={frame}
      className={styles["frame"]}
      title={title}
      sandbox={SANDBOX}
      referrerPolicy="no-referrer"
      loading="lazy"
      srcDoc={srcDoc}
      style={{ height: `${String(measured ?? estimateHeight(html))}px` }}
    />
  );
}
