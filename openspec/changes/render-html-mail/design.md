## Context
Email bodies are untrusted HTML. Two layers keep them inert: the server
sanitises (`conversations::domain::sanitize`, ammonia allow-lists) and the
client shows the result only in a sandboxed `srcdoc` frame with its own CSP
(`lib/ui/safe-html`). The frame inherits the page's policy (`default-src
'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self'
data: blob: https:`), so in the frame inline `style` attributes work and
`<style>` elements never do.

Evidence (fixtures under `crates/modules/conversations/fixtures/email-html`,
rendered raw and through the old pipeline in Chromium and WebKit at 375px and
1280px; numbers from the old frame vs. the real content height):

| Fixture | Old frame | Content | Off by |
| --- | --- | --- | --- |
| gmail (343px) | 875 | 607 | padded 268 |
| marketing-fluid (343px) | 1400 | 1841 | cut 441 |
| outlook (343px) | 806 | 860 | cut 54 |
| receipt (720px) | 1174 | 1025 | padded 149 |
| marketing-600 (343px) | 1198 | 1033, 616px wide | padded, cut sideways |

Every fixture styled by a sheet lost its layout; the newsletter showed its
`<title>` and hidden preheader as text; the hostile fixture loaded two
tracking pixels after "Show images" (sized and hidden by style).

## Decisions

### Data, events, scopes, AI
No data ownership changes: nothing is stored, no migration, no event. The one
endpoint touched, `conversations.get_message_body`, keeps its scope
(`conversations:read`) and access check; it gains an optional `css` field.
No AI is involved, and rendering reads no meaning from content: the CSS and
HTML rules here are structural (which tokens may load something), never
about what a message says.

### Style sheets: inline them on the server
Mail is written for clients that drop `<style>` (Gmail did until 2016,
Outlook's web and mobile clients still strip much of it), so senders inline
their own CSS and the industry tool for it, a CSS inliner, is the proven
path. Before ammonia runs, `styles::inline` uses css-inline 0.22 (MIT, on
html5ever and Servo's `selectors`, the same html5ever and cssparser ammonia
already uses) to apply the sheets' rules to `style` attributes, with
`load_remote_stylesheets(false)`, no default features (no `http`, no
`file`) and a resolver that refuses every location, so a `<link>` or
`@import` is never fetched or read. Sheets over 256 KiB are dropped as
before (selector matching grows with rules times elements). On any inliner
error the HTML goes on without its sheets, as before.

Alternatives considered:
- *Keep `<style>` and let the frame apply it*: needs `'unsafe-inline'` (or a
  per-response nonce) in the page's `style-src`, which weakens the page's
  policy for the whole app. Rejected.
- *Hand the whole sanitised sheet to the client as a constructed style sheet
  (no inlining)*: keeps every rule (pseudo-classes too), but a pixel hidden by
  a class could no longer be spotted on the server, and mail would lose all
  its styling whenever the frame cannot be reached. Kept only for what
  inlining cannot express (below).
- *Inline on the client*: puts a CSS engine and the trust decision in the
  browser and duplicates the server's filter. Rejected.

What inlining loses: pseudo-classes (`:hover`), pseudo-elements, and at-rules.
`@media` rules are kept another way (below); dark-mode rules are dropped on
purpose (mail is shown on a white page, as written for light).

### Every declaration through a real CSS parser
The old filter dropped a declaration when its lower-cased text contained a
deny-listed substring. `css::declarations` now reads `style` attributes (and
rule bodies) with cssparser and keeps a declaration only when every token of
its value is inert: functions only from an allow-list (colours, `calc`/`min`/
`max`/`clamp`, gradients, transforms, filters, timing, shapes, counters,
grid), no `url(` token, no at-keyword, no malformed URL or string, no escape
(`\`) and no `<` anywhere in the source, nesting at most 8 deep. Custom
properties (and so `var()`), `behavior`, `-moz-binding` and viewport units
are dropped (the frame is sized to its content, and a length that follows the
frame's height would grow it without end). A function the parser does not
know is refused, so a future loading function (`src()`, `image()`) is
refused by default.

### `@media` rules: filtered on the server, applied to the frame by the client
Responsive mail relies on `@media (max-width: …)` with `!important` to beat
its own inlined styles. css-inline keeps at-rules back; `css::media_rules`
reads them with cssparser and keeps only `@media` blocks whose query is
plain (no functions, URLs or escapes, nothing about `prefers-color-scheme`)
and, inside them, style rules whose selectors use no URLs, escapes or
functions beyond the structural pseudo-classes, with the same declaration
filter. The result (at most 64 KiB) goes to the client in `css`, outside the
HTML: it is never put in markup. The client applies it with
`document.adoptedStyleSheets` on the frame's document only. A constructed
sheet is CSSOM, which `style-src` does not govern (verified in Chromium and
WebKit: the page's policy still blocks a `<style>` element in the same
frame), so the page's policy is unchanged; the loads a rule could name stay
under the frame's own `default-src 'none'; img-src data:` (and the server
removed every `url(` anyway). `class` attributes are kept so the rules can
match; nothing else reads them (no script runs in the frame).

### The frame is measured, not estimated
An estimate from markup cannot know widths, fonts, inlined padding or images,
and the evidence shows it wrong in both directions for every fixture. The
frame needs to be measured, and an opaque-origin frame cannot be read from
outside. Options:

1. *`allow-same-origin` without `allow-scripts`* (chosen): the parent reads
   the frame's `documentElement` size, observes it (`ResizeObserver`, for
   late images and width changes), applies the `@media` sheet, and sets the
   frame's height.
2. *`allow-scripts` with a measuring script that posts its height*: needs a
   script allowed by both policies (the page's `script-src 'self'` plus an
   exact path in the frame's policy) and makes script execution in the mail
   frame possible at all; a sanitiser bypass plus a policy slip would then
   run attacker code. Rejected: the one invariant worth keeping absolute is
   that nothing ever runs in the frame.
3. *Render off-screen in the app's document to measure*: untrusted markup in
   the app's own document (and its image policy allows `https:`). Rejected.
4. *A fixed tall frame that scrolls inside*: nested scrolling, the current
   complaint. Rejected.

Threat analysis of option 1. The frame keeps `sandbox="allow-same-origin
allow-popups allow-popups-to-escape-sandbox"` with no `allow-scripts`:

- **Script execution** stays impossible by three independent layers: the
  sandbox's scripting flag (browser-enforced: no `<script>`, no handlers, no
  `javascript:` URLs, whatever the markup), the inherited page policy
  (`script-src 'self'`, no inline) and the frame's own `default-src 'none'`;
  and the sanitiser removes scripts first. With no script in the frame, sharing
  the origin gives the frame's content nothing to act with: it cannot read
  the parent, cookies (also `HttpOnly`) or storage, or call the API.
- **Forms, top navigation, plugins**: still blocked by the sandbox (no
  `allow-forms`, `allow-top-navigation`) and removed by the sanitiser.
- **Credentialed requests to the app's origin**: a same-origin frame's
  subresource loads and link clicks carry the `SameSite=Strict` session
  cookie, where an opaque frame's might not. Before "Show images" the frame
  loads nothing remote (`img-src data:`); after, an image can point at the
  app's own origin and make a cookie-carrying GET. Every GET is a read (`cargo
  xtask arch` checks that operations that mutate are not GETs; the BFF
  refuses unsafe methods without the CSRF header), and the response is
  never readable by the frame. Links open a new top-level tab with
  `noopener noreferrer`, which any user could open by hand.
- **Regression risk**: if `allow-scripts` were ever added next to
  `allow-same-origin`, the frame could reach the app. The client test asserts
  the exact sandbox token set and that `allow-scripts` is absent, and the
  component documents why.
- **Parent code**: the app only reads layout from the frame and writes
  `zoom` and an adopted sheet; it never reads content or executes anything
  from it.

### Fit: shrink a layout wider than the reader
After the `@media` rules apply, a layout still wider than the frame (a fixed
600px sale at 375px) is scaled with CSS `zoom` on the frame's body to fit,
as iOS Mail and the Gmail app do, down to half size; past that it scrolls
sideways inside its frame, never the page. Zoom (standard in Chromium,
WebKit and Firefox) changes layout size, so the measured height is the
shrunk one. The frame body's `overflow-wrap` becomes `break-word` (was
`anywhere`, which let a table squeeze its columns until words broke mid-way;
`break-word` keeps table min-widths intact and lets zoom fit the result).

### Pixels by style
A tracking pixel used to be recognised by its `width`/`height` attributes or a
substring in its style. Now the (filtered) style is parsed too: 2 px or less
in both directions by attribute or style, `max-width`/`max-height` of 0,
`display: none`, `visibility: hidden|collapse` or `opacity` 0 make an image a
pixel, removed whether or not images are shown.

## Risks / Trade-offs
- **css-inline is new code on the path of every HTML body.** It parses with
  the same html5ever as ammonia, and ammonia still cleans everything after it,
  so a parsing difference can cost styling, never safety. Pinned to an exact
  version (0.22.0, two weeks old), no default features.
- **Inlining grows the HTML** (every matched declaration repeated per
  element). Bodies are already capped at 2 MiB read; the sheet cap bounds the
  work.
- **CSSOM vs CSP.** The design relies on constructed sheets not being
  governed by `style-src` (true in the CSP spec and in Chromium and WebKit
  today). If an engine started enforcing it, the `@media` rules would simply
  not apply; inlined styles would still.
- **Lost fidelity**: `:hover`, CSS background images, web fonts, dark-mode
  variants.

## Desktop shell (finding, not changed here)
The Tauri shell's production CSP (`default-src 'self'` with no `style-src-attr`
or `img-src`) is inherited by the frame too. In it, every inline style (even
the frame's own body style), every `data:` image (placeholders, inline `cid:`
images) and every remote image after "Show images" is refused: simulated in
WebKit with that policy, the old frame logs 8 to 51 refusals per fixture and
shows unstyled Times text. Matching the web page's policy
(`style-src-attr 'unsafe-inline'; img-src 'self' data: blob: https:`) would
fix it, but widens the desktop page's own policy, so it is left to a
separate, explicit decision.

## Remote images: options for the owner (not built)
1. *Held back until "Show images"* (today, kept): nothing loads before
   consent; opening reveals the reader's IP and time to senders afterwards.
2. *Proxy at view time*: the server fetches images when the member shows
   them; senders see the server's IP, not the reader's, still at open time.
3. *Fetch at sync time and store* (Gmail's model, roughly): images shown by
   default and opening reveals nothing, but the server fetches every image of
   every message (cost, storage, SSRF surface to guard, and the fetch itself
   tells senders the message arrived).
4. *Per-sender trust*: show images automatically for senders the member
   allowed; combinable with 1 or 2.
