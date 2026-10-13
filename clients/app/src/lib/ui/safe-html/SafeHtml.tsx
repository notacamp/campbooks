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
  "overflow-wrap: anywhere",
  "color-scheme: light",
].join("; ");

/** The document the frame shows. */
export function frameDocument(html: string, remoteImages: boolean): string {
  return [
    "<!doctype html><html><head>",
    '<meta charset="utf-8">',
    `<meta http-equiv="Content-Security-Policy" content="${framePolicy(remoteImages)}">`,
    '<meta name="referrer" content="no-referrer">',
    '<base target="_blank">',
    `</head><body style="${BODY_STYLE}">`,
    html,
    "</body></html>",
  ].join("");
}

/** Smallest and largest frame heights, in px; longer mail scrolls inside. */
const MIN_HEIGHT = 96;
const MAX_HEIGHT = 1400;

/**
 * A height for the frame from the markup alone: a frame with no scripts and
 * no same-origin access cannot be measured from outside, so it is sized
 * from its text and declared image heights, and scrolls past that.
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

export interface SafeHtmlProps {
  /** HTML the server sanitised. Still untrusted: shown only in the sandbox. */
  html: string;
  /** Accessible name of the frame ("Message from Maya Okafor"). */
  title: string;
  /** Let remote images load (the member chose "Show images"). */
  remoteImages?: boolean | undefined;
}

/**
 * Untrusted HTML (an email body) in a sandboxed frame: no scripts, an
 * opaque origin (no same-origin access either way), its own strict policy,
 * and links that can only open a new tab without an opener. The only place
 * the app renders HTML it did not write (AGENTS.md).
 */
export function SafeHtml({ html, title, remoteImages = false }: SafeHtmlProps) {
  return (
    <iframe
      className={styles["frame"]}
      title={title}
      sandbox="allow-popups allow-popups-to-escape-sandbox"
      referrerPolicy="no-referrer"
      loading="lazy"
      srcDoc={frameDocument(html, remoteImages)}
      style={{ height: `${String(estimateHeight(html))}px` }}
    />
  );
}
