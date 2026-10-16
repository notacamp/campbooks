## Why
HTML email does not render properly in the reader (campbooks-cloud#160). The
owner: "It should protect against js injection, pixels and stuff, but still
render html." A fixture set of real-world-shaped mail (a newsletter styled by
a head style sheet, a receipt in nested tables, Outlook and Gmail replies, a
fixed 600px sale, a fluid marketing layout and a hostile message), rendered
raw and through the current pipeline at 375px and desktop, shows what breaks:

- Most mail is styled by a `<style>` sheet and classes. The sanitiser drops
  `<style>` and `class`, so newsletters and receipts lose their layout,
  buttons become plain links and hidden preheaders become visible text. The
  page's policy (`style-src 'self'`, inherited by the `srcdoc` frame) would
  block a kept `<style>` anyway.
- A document's `<title>` leaks into the body as the first line.
- The frame's height is estimated from the markup, independent of its width:
  every fixture was cut short (by up to 441px) or padded (by up to 337px),
  and mail past 1400px scrolled inside its frame.
- A fixed 600px layout does not fit 375px: it is cut off at the frame's edge.
- An image sized 1×1 or hidden by its *style* (not its attributes) slipped
  past the tracking-pixel check and loaded after "Show images".

## What Changes
- The sanitiser inlines a message's style sheets into `style` attributes
  before cleaning (css-inline, with remote and file loading off), keeps
  `class`, drops `<title>` with its content, and moves the `<body>`'s own look
  to a wrapper.
- `style` declarations are filtered with a real CSS parser (cssparser) and an
  allow-list of functions instead of substring checks: nothing that loads or
  runs survives (`url(`, `image-set(`, `src(`, `expression(`, `var(`, `attr(`,
  escapes, custom properties, viewport units).
- `GET …/messages/{message_id}/body` also returns the message's `@media`
  rules, filtered the same way, in a new `css` field.
- Tracking pixels are also recognised by their style (1×1 or `max-width`/
  `max-height: 0`, `visibility: hidden`, `opacity: 0`).
- The reader's frame shares the app's origin (`allow-same-origin`, never
  `allow-scripts`) so it can be measured: it is as tall as its content, a
  layout wider than the reader is shrunk to fit (down to half size), and the
  `@media` rules apply to the frame's document as a constructed style sheet.

## Capabilities
- Modified: `conversations` (Sanitised email HTML, Reading conversations),
  `inbox-ui` (Reading a conversation).

## Impact
Module: `conversations` (server: `domain::sanitize`, `get_message_body`;
client: `lib/ui/safe-html`, the reader). New dependency `css-inline` (MIT,
no default features); `cssparser` and `html5ever` become direct dependencies
(already in the graph through ammonia); `selectors` and `cssparser-macros`
(MPL-2.0, Servo's, used unmodified) join the scoped licence exceptions. No migration, no event, no
new operation; one new optional response field. The page's CSP is unchanged.
Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/160

## Non-goals
- An image proxy (fetching remote images through the server). Remote images
  stay held back until "Show images"; the options are listed for the owner.
- CSS background images: `url()` is dropped even after "Show images".
- Pseudo-classes (`:hover`) and other rules that cannot be inlined, apart from
  `@media` rules; dark-mode rules (`prefers-color-scheme`), since mail is shown
  on a white page.
- The desktop (Tauri) shell's own CSP, which blocks inline styles and `data:`
  images in the frame: a separate decision (see design.md).
- Reading meaning from mail: rendering decides nothing from its text.
