## ADDED Requirements

### Requirement: Access is asked again on every page
An open stream SHALL decide container visibility afresh for every page of
changes it reads, never from an earlier page. It SHALL resolve the caller's
membership before showing a page that changes who belongs to the
workspace. When a container whose changes it has shown is no longer
visible, it SHALL send `reset` with reason `access_changed` (the client
drops what it cached and refetches) and show nothing more from that
container. Every heartbeat SHALL ask again too.

#### Scenario: Taken out of an inbox
- **WHEN** a member following the feed is removed from an inbox and new mail arrives in it
- **THEN** they receive `reset` (`access_changed`) and no id of the new mail

#### Scenario: Removed from the workspace
- **WHEN** a member following the feed is removed from the workspace
- **THEN** the stream sends `revoked` and ends, without the change that removed them

### Requirement: Private records reach their audience only
A change MAY name an audience: one user, one user and the workspace's
owners, admins and client tokens, or client tokens only. A stream SHALL show
such a change only to that audience. Draft changes SHALL reach only their
author (or client tokens, for a draft a client wrote). Connection changes
SHALL reach only the member who connected it, owners, admins and client
tokens.

#### Scenario: Another member's draft
- **WHEN** a sender of an inbox sends a draft
- **THEN** the inbox's other members and the workspace's admins do not receive its change
