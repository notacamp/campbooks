import { useEffect, useMemo, useRef, useState } from "react";

import { MAIL_PAGE } from "../tokens/tokens";

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

/** The attribute the server wraps a reply's trailing quoted history in. */
const QUOTED_MARK = "data-cb-quoted";

/** How the fold's summary looks: a small dashed pill on the mail's white page. */
const SUMMARY_STYLE = [
  "display: inline-block",
  "margin: 8px 0",
  "padding: 1px 10px",
  `border: 1px dashed ${MAIL_PAGE.foldBorder}`,
  "border-radius: 999px",
  `color: ${MAIL_PAGE.foldText}`,
  "font-size: 12px",
  "line-height: 1.6",
  "cursor: pointer",
  "list-style: none",
].join("; ");

/**
 * The frame's own rules (a constructed sheet: the page's policy blocks
 * `<style>`): the fold's summary says Hide once it is open.
 */
const FRAME_RULES = [
  `details[${QUOTED_MARK}] > summary::-webkit-details-marker { display: none; }`,
  `details[${QUOTED_MARK}][open] > summary > [data-cb-show] { display: none !important; }`,
  `details[${QUOTED_MARK}][open] > summary > [data-cb-hide] { display: inline !important; }`,
].join("\n");

/**
 * Fold the quoted history the server marked (one `div[data-cb-quoted]`)
 * behind a `<details>`, which opens and closes with no script. The mail
 * cannot write the mark: the server lets no `data-` attribute through.
 */
function foldQuoted(doc: Document, show: string, hide: string): void {
  const block = doc.querySelector("div[data-cb-quoted]");
  if (block === null) {
    return;
  }
  const details = doc.createElement("details");
  details.dataset["cbQuoted"] = "";
  const summary = doc.createElement("summary");
  summary.setAttribute("style", SUMMARY_STYLE);
  const showLabel = doc.createElement("span");
  showLabel.dataset["cbShow"] = "";
  showLabel.textContent = show;
  const hideLabel = doc.createElement("span");
  hideLabel.dataset["cbHide"] = "";
  hideLabel.setAttribute("style", "display: none");
  hideLabel.textContent = hide;
  summary.append(showLabel, hideLabel);
  block.replaceWith(details);
  details.append(summary, block);
}

/**
 * Take the remote images' sources out of a document: every image source but
 * a `data:` one (the server's placeholder and a message's own parts), for
 * markup sent with its remote images that the reader holds back after all.
 * The frame's policy blocks them already; without a source nothing is even
 * asked for.
 */
function stripRemoteImages(doc: Document): void {
  for (const image of doc.querySelectorAll("img")) {
    const src = image.getAttribute("src");
    if (src !== null && !src.trim().toLowerCase().startsWith("data:")) {
      image.removeAttribute("src");
    }
    image.removeAttribute("srcset");
  }
}

/** How {@link prepareHtml} prepares the mail for the frame. */
interface PrepareOptions {
  inertLinks: boolean;
  quoted?: { show: string; hide: string };
  /** Remote images held back from markup sent with them ({@link stripRemoteImages}). */
  holdImages?: boolean;
}

/**
 * `html` as the frame shows it, parsed inertly (a `DOMParser` document
 * loads and runs nothing): its links taken out ({@link stripLinks}) when
 * `inertLinks`, its quoted history folded ({@link foldQuoted}) when labels
 * are given, its remote images' sources taken out when `holdImages`.
 */
export function prepareHtml(
  html: string,
  { inertLinks, quoted, holdImages = false }: PrepareOptions,
): string {
  if (!inertLinks && quoted === undefined && !holdImages) {
    return html;
  }
  const doc = new DOMParser().parseFromString(html, "text/html");
  if (inertLinks) {
    stripLinks(doc);
  }
  if (holdImages) {
    stripRemoteImages(doc);
  }
  if (quoted !== undefined) {
    foldQuoted(doc, quoted.show, quoted.hide);
  }
  return doc.body.innerHTML;
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
  const own = new view.CSSStyleSheet();
  own.replaceSync(FRAME_RULES);
  if (css === undefined || css === "") {
    doc.adoptedStyleSheets = [own];
    return;
  }
  const sheet = new view.CSSStyleSheet();
  sheet.replaceSync(css);
  doc.adoptedStyleSheets = [own, sheet];
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
  /**
   * The markup came with its remote images' sources, but they are held back
   * after all (the reader is checking a sender's trust again): the sources
   * are taken out before the frame parses the mail.
   */
  holdRemoteImages?: boolean | undefined;
  /** The message's `@media` rules, filtered by the server; frame only. */
  css?: string | null | undefined;
  /** Mail that may be phishing: its links lead nowhere. */
  inertLinks?: boolean | undefined;
  /**
   * "Show quoted text": fold the quoted history the server marked behind
   * this label (with {@link SafeHtmlProps.hideQuoted} once open).
   */
  showQuoted?: string | undefined;
  /** "Hide quoted text". */
  hideQuoted?: string | undefined;
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
  holdRemoteImages = false,
  css,
  inertLinks = false,
  showQuoted,
  hideQuoted,
}: SafeHtmlProps) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [measured, setMeasured] = useState<number | undefined>();
  // Inert links are taken out before the frame ever parses the mail (an
  // inert DOMParser document loads and runs nothing), and again on load.
  const srcDoc = useMemo(
    () =>
      frameDocument(
        prepareHtml(html, {
          inertLinks,
          holdImages: holdRemoteImages,
          ...(showQuoted === undefined
            ? {}
            : { quoted: { show: showQuoted, hide: hideQuoted ?? showQuoted } }),
        }),
        remoteImages && !holdRemoteImages,
      ),
    [html, remoteImages, holdRemoteImages, inertLinks, showQuoted, hideQuoted],
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
