## MODIFIED Requirements

### Requirement: Reading a conversation
The reader SHALL show the conversation's subject, the note, and its messages
newest first with the newest (and any unread) open and older ones folded to
one line until opened, each with sender, recipients, time and attachments.
HTML bodies SHALL be shown only in a sandboxed frame that never runs scripts
(sandbox exactly `allow-same-origin allow-popups
allow-popups-to-escape-sandbox`: no `allow-scripts`, no forms, no top
navigation), with its own Content-Security-Policy and no referrer. The frame
SHALL be as tall as its content (measured, and again when its width or
content changes; never scrolling inside except sideways), SHALL shrink a
layout wider than the reader to fit, down to half size, and SHALL apply the
message's filtered `@media` rules to its own document only, never to the
app's page. Remote images SHALL stay blocked until the user chooses "Show
images" for that message (Shift+I for all). Opening an unread conversation
SHALL mark it read.

#### Scenario: A newsletter with remote images
- **WHEN** the user opens a message whose body has a remote image
- **THEN** the frame's policy allows only `data:` images, the reader says one remote image is hidden, and after "Show images" the frame allows `https` images

#### Scenario: A long message
- **WHEN** the user opens a message whose content is 3,000px tall
- **THEN** the frame is 3,000px tall, cut nowhere and not padded, and the reader scrolls, not the frame

#### Scenario: A fixed 600px layout on a phone
- **WHEN** the user opens a message laid out at 600px wide at 375px
- **THEN** it is shown shrunk to the frame's width and the page does not scroll sideways

#### Scenario: A responsive newsletter on a phone
- **WHEN** a message's `@media (max-width: 620px)` rule makes its container full width and the frame is 343px wide
- **THEN** the rule applies inside the frame, the layout fits without shrinking, and the app's own page is unaffected
