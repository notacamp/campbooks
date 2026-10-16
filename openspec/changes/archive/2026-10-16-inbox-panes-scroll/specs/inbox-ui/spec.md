# Delta for inbox-ui

## MODIFIED Requirements

### Requirement: Separate screens on phones
From 64rem the list and the reader SHALL show side by side at every width,
the reader taking the width the sidebar and the list leave and never wrapping
under the list, and the page SHALL be exactly the window's height: the
sidebar, the list and the reader each scroll on their own, a pane's header
staying at the top of its pane; below 64rem they SHALL be separate screens, the reader with
"Back to <stream>" (Esc), and below 48rem the reader's Done (and Reply) SHALL
float above the tab bar. Every screen SHALL exist in en, pt-PT, es-ES and fr,
show loading, empty and error states (including no mailbox and a conversation
that is gone), fit 375px without horizontal scrolling and pass axe in light
and dark. No content (a long sender address, subject, URL or word) SHALL make
a screen wider than the viewport at any width; long unbroken text wraps.

#### Scenario: Opening a conversation on a phone
- **WHEN** the user taps a conversation at 375px
- **THEN** the reader replaces the list, and Back returns to the stream

#### Scenario: A narrow desktop window
- **WHEN** a conversation is open at 1100px wide
- **THEN** the reader sits beside the list, its top level with the list's, and the page is no wider than the window

#### Scenario: Long unbroken text
- **WHEN** an open conversation's subject and sender have no spaces or hyphens and are longer than the reader is wide
- **THEN** the text wraps inside the reader and the page does not scroll sideways, at 375px, 1100px and 1440px

#### Scenario: A full inbox on desktop
- **WHEN** a stream holds more conversations than fit in a 1100px or 1440px window and one is open
- **THEN** the page is no taller than the window, the list scrolls inside its pane with its tabs kept in view, and the reader stays in view beside it
