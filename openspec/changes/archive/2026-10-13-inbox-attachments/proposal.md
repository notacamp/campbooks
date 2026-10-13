## Why

The reader lists a message's attachments by name, but nothing can open
them: there is no download operation. Remote images that the reader holds
back show the browser's broken-image icon, and inline (`cid:`) images,
which are part of the message rather than anything remote, vanish.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/105
(follow-up of #100, notacamp/campbooks#459).
Modules touched: `conversations` (server crate and client module),
`connectors` (the Gmail parser keeps each part's `Content-ID`), the
server's web delivery (its Content-Security-Policy), and `lib/ui` (new
components). No new module, no cross-module dependency, no new crate.

## What Changes

- **Download operation** `conversations.get_attachment`
  (`GET /api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/attachments/{attachment_id}`,
  scope `conversations:read`, inbox-membership gated, 404 for anything the
  caller cannot see). The bytes stream from blob storage with
  `Content-Disposition: attachment` and a sanitised file name, the stored
  content type only when it is on a safe list (otherwise
  `application/octet-stream`), `nosniff`, `Content-Security-Policy:
  sandbox`, `Cross-Origin-Resource-Policy: same-origin` and `no-store`.
  `?inline=true` is honoured only for raster images and PDFs.
- **Previews**: images open in the app in a dialog, as an `<img>` of a
  `blob:` URL whose type the client sets from a raster allow-list (never
  as HTML); PDFs open in the browser's own viewer in a new tab, served
  inline under `Content-Security-Policy: sandbox`. HTML, SVG and anything
  else are download-only.
- **Inline `cid:` images** are shown always, as `data:` URLs the server
  builds from the stored part (raster types only, bounded in size). The
  connectors' Gmail parser now keeps each part's `Content-ID`
  (MessageChangeBatch v1 gains an optional `content_id`; the conversations
  schema gains `attachments.content_id`).
- **Blocked-image placeholder**: a held-back remote image keeps its size and
  alt text and shows a fixed design-system placeholder (a `data:` SVG the
  server puts in place) instead of the browser's broken-image icon. It
  loads nothing remote.
- **UI**: an attachment list under each open message (type icon, name,
  size, Download, Preview), V to move through the attachments of the open
  message and Enter to open one, an image preview dialog; en, pt-PT, es-ES,
  fr; 375px and desktop; Storybook stories.
- **Web CSP** gains `blob:` in `img-src` for the image preview.
- **Fake Google**: the demo mailbox gains a message with a photo, an inline
  logo, a remote image and an HTML attachment.

## Non-goals

- Range requests (`Accept-Ranges: none`; see design).
- Previews in the native (Tauri) client: its bridge carries JSON only, so
  the attachment actions are web-only for now.
- A fetch-on-demand path: the Gmail adapter stores every attachment with
  its message (messages over the size limit are skipped whole), so a
  listed attachment always has its bytes.
- Backfilling `content_id` for messages synced before this change (their
  inline images keep showing the placeholder until the message is synced
  again).
- Office documents, text or video previews; an image proxy; virus scanning.
