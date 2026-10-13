## ADDED Requirements

### Requirement: Attachment downloads
`conversations.get_attachment` (`GET
/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/attachments/{attachment_id}`,
scope `conversations:read`) SHALL stream an attachment's stored bytes to a
caller who can see the message's inbox, and SHALL answer 404 when the
conversation, message and attachment ids do not belong together, when the
caller cannot see the inbox, or when the bytes are gone. The response SHALL
carry `Content-Disposition: attachment` with a sanitised file name (no path
separators, control or bidirectional-override characters, no leading dots,
at most 150 characters, `attachment` when nothing is left) as both an ASCII
`filename` and a UTF-8 `filename*`; the stored content type only when it is
on the safe list, else `application/octet-stream`; `X-Content-Type-Options:
nosniff`; `Content-Security-Policy` starting with `sandbox`;
`Cross-Origin-Resource-Policy: same-origin`; `Cache-Control: private,
no-store`; and `Accept-Ranges: none`. With `inline=true` the disposition
SHALL be `inline` only for raster images and PDFs; every other type stays
an attachment.

#### Scenario: A member downloads a PDF
- **WHEN** a member of the inbox requests a message's PDF attachment
- **THEN** the answer is 200 with the stored bytes, `Content-Type: application/pdf`, `Content-Disposition: attachment; filename="…"; filename*=UTF-8''…`, `nosniff` and a `sandbox` policy

#### Scenario: Another inbox
- **WHEN** a member of the workspace who is not in the message's inbox requests the attachment
- **THEN** the answer is 404, the same as for an id that does not exist

#### Scenario: An HTML attachment
- **WHEN** a member requests an attachment stored as `text/html`, with `inline=true`
- **THEN** it is served as `application/octet-stream` with `Content-Disposition: attachment`

#### Scenario: A malicious file name
- **WHEN** an attachment is named `../../etc/pass\r\nwd‮gpj.exe`
- **THEN** the header names it `etcpasswdgpj.exe`, with no separators, line breaks or bidirectional controls, as one valid header

## MODIFIED Requirements

### Requirement: Sanitised email HTML
`conversations.get_message_body` SHALL sanitise the HTML body before
returning it: only an allow-list of tags and attributes survives; `script`
and `style` elements are removed with their content, as are forms, frames,
objects, `base` and `meta` elements, event-handler attributes and comments;
links keep only `http`, `https`, `mailto` and `tel` URLs, relative URLs are
dropped, and every link opens in a new tab with `rel="noopener noreferrer"`;
`style` attributes keep only declarations that load nothing (no `url(`,
`image-set(`, `@import`, `expression`, `behavior`, `-moz-binding` or CSS
escapes). Images of at most 2×2 pixels, or hidden by their style, SHALL be
removed. Other remote images SHALL have their source replaced by the
server's placeholder image (a fixed `data:` URL; their size and alt text
kept) unless the request asks `show_images=true`, and the response SHALL say
how many remote images the message has (`remote_images`) and whether they
were kept (`images_shown`). An image whose source is `cid:<id>` SHALL be
shown, whether or not images are shown, as a `data:` URL of the message's
inline part with that `Content-ID` when that part has a raster image type
and fits the size limits, and as the placeholder otherwise. No other `data:`
source survives.

#### Scenario: A newsletter with a script, a style sheet and a tracking pixel
- **WHEN** a member reads the HTML body of a message that has a `<script>`, a `<style>` with a remote `url()`, an `onclick` handler, a 1×1 image and a remote logo
- **THEN** the HTML has no script, style sheet, handler or tracking pixel, the logo's source is the placeholder, and `remote_images` is 1

#### Scenario: Show images
- **WHEN** the same body is read with `show_images=true`
- **THEN** the logo keeps its `https` source, the tracking pixel is still removed, and `images_shown` is true

#### Scenario: An inline logo
- **WHEN** a member reads a message whose HTML shows `cid:logo@example.com` and which has an inline PNG part with that content id
- **THEN** the image's source is a `data:image/png;base64,…` URL of that part, without asking to show images, and it is not counted in `remote_images`
