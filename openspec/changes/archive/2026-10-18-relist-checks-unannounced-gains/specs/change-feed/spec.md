## MODIFIED Requirements

### Requirement: Access is asked again on every page
An open stream SHALL show a page only under the containers the caller may
see as listed after every change of who sees inside a container that was
numbered up to that page: it SHALL keep the list it made and list them
again before showing a page whose range holds such a change (a change of a
record type the container-visibility port names as changing access,
whether or not the caller may see it), and at least once a heartbeat
interval. When any new list (at a heartbeat, after a change of access, or
after a change about some other container) holds a container the previous
list lacked and that no change behind that list named (as its record or
its container), a gain nothing announced, it SHALL send `reset` with reason
`access_changed`; a gain a change announced needs none. It SHALL resolve the caller's membership before
showing a page when a change that may change who belongs to the workspace
was numbered after what it last looked at, up to the head the read saw,
whether or not the caller may see that change. When a container whose
changes it has shown is no longer visible, it SHALL send `reset` with
reason `access_changed` (the client drops what it cached and refetches)
and show nothing more from that container. Every heartbeat SHALL ask again
too. Each container of a page SHALL be asked about once more before the
page is shown, and a change in one that is no longer visible SHALL NOT be
shown.

#### Scenario: Taken out of an inbox
- **WHEN** a member following the feed is removed from an inbox and new mail arrives in it
- **THEN** they receive `reset` (`access_changed`) and no id of the new mail

#### Scenario: Removed from the workspace
- **WHEN** a member following the feed is removed from the workspace
- **THEN** the stream sends `revoked` and ends, without the change that removed them

#### Scenario: A removal the member may not see
- **WHEN** a member's membership ends and the only membership change recorded is one they may not see
- **THEN** the stream sends `revoked` and ends before its next heartbeat

#### Scenario: Given an inbox
- **WHEN** an inbox is made for a member who follows the feed and mail arrives in it
- **THEN** they receive the new mail's changes before their next heartbeat

#### Scenario: Given an inbox nothing announced
- **WHEN** an inbox is made for a member who follows the feed without a change announcing it, and mail arrives in it
- **THEN** at their next heartbeat they receive `reset` (`access_changed`), and the inbox's later mail reaches them

#### Scenario: Listed again for another inbox
- **WHEN** an inbox nothing announced is made for a member, its mail is read past, and then a change about an inbox they cannot see is numbered
- **THEN** the listing that change causes sends `reset` (`access_changed`) before their next heartbeat, and an inbox later granted with its announcement sends none

#### Scenario: Removed after more than a page
- **WHEN** more than a page of changes a member may see and then their removal are numbered together
- **THEN** the stream sends `revoked` and ends without showing any of those changes

#### Scenario: Woken by changes that do not touch access
- **WHEN** a stream is woken again and again by mail and renames
- **THEN** it lists the caller's containers once, not once per wake-up
