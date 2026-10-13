## ADDED Requirements

### Requirement: Attachments
Each open message SHALL list its attachments, except inline images shown in
its body, with a type icon, the file name (bidirectional and control
characters removed), the size, a Download action and, for raster images and
PDFs, a Preview action. An image preview SHALL show the picture as an image
element of a `blob:` URL whose type the client sets from its raster list,
in a dialog with Download and Close; a PDF preview SHALL open the
attachment in a new tab, served inline under a sandbox policy. HTML, SVG and
every other type SHALL be download-only. V SHALL move focus through the
open message's attachments, wrapping, and Enter SHALL open the focused one;
the list SHALL show the V hint.

#### Scenario: Preview a photo
- **WHEN** the member presses V on a message with a JPEG attachment, then Enter
- **THEN** a dialog shows the photo with its name, Download and Close, and Escape closes it

#### Scenario: An HTML attachment
- **WHEN** a message has an `invoice.html` attachment
- **THEN** its row offers Download only

### Requirement: Placeholder for blocked images
A remote image held back in an email body SHALL show the design system's
placeholder at its declared size instead of the browser's broken-image
icon, and SHALL load nothing until the member chooses "Show images".

#### Scenario: A newsletter banner
- **WHEN** the member opens a newsletter with a remote banner and has not shown images
- **THEN** the banner's place shows the placeholder, and no request leaves for the banner's host
