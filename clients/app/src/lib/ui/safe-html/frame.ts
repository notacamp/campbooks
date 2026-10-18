// The document the mail frame shows, and its sandbox: plain strings with no
// imports, so the WebKit policy test and the recording build exactly the
// frame the app renders (campbooks-cloud#199).

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

/** The frame's referrer policy: a link or image never tells where it was seen. */
export const REFERRER_POLICY = "no-referrer";

/**
 * The sandbox for mail that may be phishing: no popups either, so a link
 * cannot open anything at all (there is no other way out of the frame).
 */
export const SANDBOX_INERT_LINKS = "allow-same-origin";

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
export function frameDocument(html: string, remoteImages: boolean): string {
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
