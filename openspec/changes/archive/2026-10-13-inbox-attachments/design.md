## Context

`conversations.attachments` already records every attachment of a message
(file name and content type as sent, size, `inline`, and a blob
reference); the Gmail adapter stores the bytes as blobs before the batch
that references them, and `conversations.get_conversation` lists them. The
kernel blob store streams a blob back verified against its SHA-256 and
says, in its module docs, that access control is the caller's and
downloads must be served as attachments with `nosniff`. Email HTML is
sanitised on the server (`domain::sanitize`) and shown only in a sandboxed
`srcdoc` frame whose own policy allows `data:` images (plus `https:` after
"Show images"); the page's policy cannot carry style sheets into the frame.

## Goals / Non-goals

Goals: download any attachment the member can see, safely; preview images
and PDFs without giving attacker-controlled bytes a way to run in the app's
origin; show inline images; replace the broken-image icon. Non-goals are in
the proposal.

## Decisions

### 1. One download operation, streamed from blobs

`conversations.get_attachment` (`GET …/conversations/{conversation_id}/messages/{message_id}/attachments/{attachment_id}`,
`conversations:read`, `Operation::query()`, not exposed to automation or
the assistant). The row must match all four ids and sit in an inbox the
caller can see (`visible_inboxes`, as every read); anything else, including
a blob that is gone, is 404. The body is `BlobStore::get`'s verified
stream; a hash mismatch aborts the response rather than serving bad bytes.

Response headers:

- `Content-Type`: the stored type's essence (before `;`, lower-cased) when
  it is on the **safe list**, else `application/octet-stream`. The list is
  types a browser does not run or render as a document of the app:
  `image/png`, `image/jpeg`, `image/gif`, `image/webp`, `image/avif`,
  `image/bmp`, `application/pdf`, `text/plain`, `text/csv`,
  `text/calendar`, `application/zip`, `application/gzip`, the Office and
  OpenDocument formats, `audio/mpeg`, `audio/ogg`, `audio/wav`,
  `video/mp4`, `video/webm`. HTML, XHTML, SVG, XML, JavaScript and every
  unknown type become `application/octet-stream`.
- `Content-Disposition: attachment; filename="<ASCII fallback>";
  filename*=UTF-8''<percent-encoded>` (RFC 6266 / 8187). The name is
  sanitised first: path separators, control characters (C0, C1, DEL) and
  bidirectional overrides (U+202A–U+202E, U+2066–U+2069, U+200E/F) are
  dropped, leading dots and surrounding whitespace trimmed, the result cut
  to 150 characters keeping its extension, and an empty name becomes
  `attachment`. `inline` replaces `attachment` only when `?inline=true`
  and the served type is a raster image or `application/pdf`.
- `X-Content-Type-Options: nosniff`, `Content-Security-Policy: sandbox;
  default-src 'none'; frame-ancestors 'none'; form-action 'none';
  base-uri 'none'`,
  `Cross-Origin-Resource-Policy: same-origin`, `Cache-Control: private,
  no-store`, `Content-Length`, and `Accept-Ranges: none`.

**Range requests are not supported.** The store verifies the whole
object's hash as it streams; a range cannot be verified without hashing
from the start, and Gmail caps a message at 25 MB, so images and PDFs are
small enough to send whole (browser PDF viewers work without ranges).
Adding ranges later means a ranged `get` in the kernel that gives up the
per-response verification; not worth it now.

**No fetch-on-demand.** The Gmail adapter downloads a message as raw MIME
and stores every part before emitting it; a message over the size limit is
skipped whole. An attachment row therefore always has its blob, so there is
no lazy path to add.

Alternatives considered:

- **Signed, expiring blob URLs** served by the kernel (no auth on the
  fetch): fewer round trips, but a new kernel capability, URLs that work for
  whoever holds them, and the store's rule is "never by URL". Rejected.
- **Serving the stored type as-is** with `nosniff`: simplest, but an
  `text/html` or `image/svg+xml` part opened in a tab would render as a
  document of the app's origin (`attachment` makes most browsers download,
  but a user can still open a downloaded HTML file from the download bar
  with the right origin in some browsers). The safe list closes that
  regardless of disposition. Chosen: the list.
- **Base64 in a JSON body** (like `get_message_body`): works through the
  native bridge too, but bloats every download by a third and holds it in
  memory; kept as the option for the Tauri follow-up.

### 2. Previews: images in the app, PDFs in the browser's viewer

Images (PNG, JPEG, GIF, WebP, AVIF, BMP) are fetched through the API
client (`parseAs: "blob"`), re-wrapped as a `Blob` whose type the client
sets from its own raster list, and shown as `<img src="blob:…">` in a
dialog. An image element only decodes images: a part that lies about its
type shows nothing, never markup. The page CSP gains `blob:` in `img-src`
for this; the email frame's own policy still allows only `data:`.

PDFs open in a new tab at the download URL with `?inline=true`, so the
browser's own viewer shows them in a top-level document whose
`Content-Security-Policy: sandbox` gives it an opaque origin with no
scripts of ours. Measured in Chromium: a PDF in a frame with the `sandbox`
attribute is refused ("blocked"), but a top-level PDF under the `sandbox`
CSP (with `default-src 'none'`, `X-Frame-Options: DENY`) renders.

Alternatives considered:

- **PDF in a sandboxed `<iframe>` in the app**: Chromium refuses to show it
  (measured). Rejected.
- **PDF in an unsandboxed in-app frame of the CSP-sandboxed response**:
  renders, but needs the global `X-Frame-Options: DENY` relaxed to
  `SAMEORIGIN` for one response type, weakening a rule every response
  shares. Rejected for now.
- **pdf.js in the app**: in-app and uniform across browsers, but a large
  new dependency with its own code-execution history (CVE-2024-4367) in the
  app's origin. Rejected.
- **PDF as a `blob:` URL in a new tab**: the blob URL carries the app's
  origin; any viewer escape would run there. Rejected.
- **Images as `<img src>` of the download URL** (no `blob:`): no CSP
  change and streamed, but it depends on the cookie reaching an image load
  and bypasses the transport's session handling; the blob path keeps every
  request on the API client. Chosen: blob.

HTML, SVG and every other type are download-only.

### 3. Inline `cid:` images: always, as `data:` URLs

An inline part is part of the message, not a remote resource: showing it
reveals nothing to the sender, so it is shown without "Show images" (as
mail clients do). The connectors' parser keeps each part's `Content-ID`
(angle brackets stripped); MessageChangeBatch v1 gains an optional
`content_id`, and `conversations.attachments` a nullable `content_id`
column. `get_message_body` loads, only when the HTML mentions `cid:`, the
message's parts that have a content id and a raster type, up to 2 MiB each
and 6 MiB in all, and hands the sanitiser a map from content id to a
`data:<type>;base64,…` URL. The sanitiser allows the `cid` scheme only for
`img[src]` (links with `cid:` are dropped) and its image policy replaces a
known `cid:` source with its `data:` URL; an unknown or oversized one
becomes the placeholder (decision 4). Every `data:` source in the output is
built by the server from stored bytes with a type from the raster list;
`data:` sources in the mail itself are still removed.

Alternatives considered:

- **Only after "Show images"**: no privacy gain (nothing remote loads), one
  more click for a logo. Rejected.
- **Frame loads the download URL**: the frame has an opaque origin and a
  policy of `img-src data:`; it would need same-origin image loads with
  cookies inside the sandbox. Rejected.
- **Client substitutes the sources**: rewriting untrusted markup in the
  browser, outside the server's sanitiser. Rejected.
- **Re-parse the raw MIME at read time** (no schema change): a 25 MB parse
  per body read. Rejected; the column is one migration.

### 4. Placeholder for blocked images

The sanitiser's image policy already sees every remote `img[src]` and
removes it. A last pass over ammonia's canonical output then gives every
image left without a source (held back, or a `data:`/relative/unknown
`cid:` source ammonia dropped) one fixed `data:image/svg+xml` URL: a
neutral panel with a small picture glyph, centred, in the design system's
light neutrals (mail is always on a white page). The SVG has no intrinsic
size, so it takes the image's declared `width`/`height` (and `alt` is
kept); an image that declares no size gets 48 × 48. It loads nothing
remote; the frame's policy allows only `data:` images. The fail-closed
check now strips every image when any `src` in a blocked render is not the
placeholder or one of this message's inline `data:` URLs.

Alternatives considered:

- **CSS in the frame** (`img:not([src])` styling): the page CSP blocks
  style sheets inside the `srcdoc` frame, and a broken image's icon cannot
  be restyled reliably. Rejected.
- **Client-side DOM rewrite** before `srcdoc`: parsing untrusted HTML in the
  app's origin. Rejected.

### 5. UI

`lib/ui` gains `AttachmentList` (name, size, type icon, Download, Preview
for images and PDFs, a busy state, an error line; sizes in kB/MB, never
"398 byte") and `ImagePreview` (an
`Overlay` dialog with the picture, its name, Download and Close). File
names shown in the app go through the same bidi/control-character
stripping. `MessageCard` takes the list in a new `files` slot. The inbox's
`ThreadMessage` hides parts shown in the body from the list:
`AttachmentView.in_body` (new) is true for an inline raster part with a
content id, so a part the body cannot show (an old message, an SVG) stays
downloadable.

Keyboard: `inbox.next_attachment` (V, item scope) moves focus through the
open message's attachments, wrapping; Enter opens the focused one (Preview
if it has one, else Download). The list shows the V hint, the shortcut
sheet lists it. V was free in every scope.

Copying the old app: the Rails app on `main` served Active Storage
downloads by signed URL and previewed PDFs in an `<iframe>`; neither is
ported. The safe-list, disposition and sandbox choices are made here on
their merits.

## Data, events, scopes

- `conversations` schema: `attachments.content_id text NULL`
  (`<ts>_conversations__attachment_content_id.sql`). No other table.
- MessageChangeBatch v1: optional `content_id` on an attachment
  (backwards compatible; absent means none).
- `AttachmentView` (in `conversations.get`) gains `in_body`.
- No events published or consumed. No AI.
- Scope: `conversations:read` for `conversations.get_attachment`.

## Risks

- Browsers other than Chromium were not measured for the sandboxed PDF tab;
  Download stays next to Preview.
- A `data:` URL inflates the body response by a third of the inline
  images' size (bounded at 6 MiB of images).
- Sync before this change leaves `content_id` empty; those messages show
  placeholders for their inline images until re-synced.
