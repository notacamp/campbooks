import { useEffect, useMemo, useRef, useState } from "react";

import { frameDocument, REFERRER_POLICY, SANDBOX, SANDBOX_INERT_LINKS } from "./frame";
import styles from "./SafeHtml.module.css";

export { framePolicy, SANDBOX, SANDBOX_INERT_LINKS } from "./frame";

/** `html` with its links made plain text ({@link stripLinks}), parsed inertly. */
export function withoutLinks(html: string): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  stripLinks(doc);
  return doc.body.innerHTML;
}

/**
 * Take the links out of a document: each `href` moves to `data-href`, so a
 * link is plain text (not focusable, no "Open in new tab", nothing the
 * mail's CSS can switch back on). The sandbox without popups stays as the
 * second layer.
 */
export function stripLinks(doc: Document): void {
  for (const link of doc.querySelectorAll<HTMLAnchorElement | HTMLAreaElement>(
    "a[href], area[href]",
  )) {
    link.dataset["href"] = link.getAttribute("href") ?? "";
    link.removeAttribute("href");
  }
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
const MAX_MEASURED_HEIGHT = 30_000;

/**
 * How many times a frame is resized for one document: enough for late images
 * and window resizes, and an end to any layout that answers its own resize.
 */
const MAX_RESIZES = 60;

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
  /** Mail that may be phishing: its links lead nowhere. */
  inertLinks?: boolean | undefined;
}

/**
 * Untrusted HTML (an email body) in a sandboxed frame: no scripts, its own
 * strict policy, links that can only open a new tab without an opener, as
 * tall as its content and fitted to its width. The only place the app
 * renders HTML it did not write (AGENTS.md).
 */
export function SafeHtml({
  html,
  title,
  remoteImages = false,
  css,
  inertLinks = false,
}: SafeHtmlProps) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [measured, setMeasured] = useState<number | undefined>();
  // Inert links are taken out before the frame ever parses the mail (an
  // inert DOMParser document loads and runs nothing), and again on load.
  const srcDoc = useMemo(
    () => frameDocument(inertLinks ? withoutLinks(html) : html, remoteImages),
    [html, remoteImages, inertLinks],
  );
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
      if (inertLinks) {
        stripLinks(doc);
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
  }, [srcDoc, rules, inertLinks]);

  return (
    <iframe
      ref={frame}
      className={styles["frame"]}
      title={title}
      // Sandbox flags only apply when a frame loads: a new frame when they change.
      key={inertLinks ? "inert" : "live"}
      sandbox={inertLinks ? SANDBOX_INERT_LINKS : SANDBOX}
      referrerPolicy={REFERRER_POLICY}
      loading="lazy"
      srcDoc={srcDoc}
      style={{ height: `${String(measured ?? estimateHeight(html))}px` }}
    />
  );
}
