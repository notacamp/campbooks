## MODIFIED Requirements

### Requirement: Backpressure and limits
The stream SHALL read changes in bounded pages of changes the caller may
see, reading the next page only once the client has taken the previous
page's frames (but for one held ready), and SHALL hold no database
connection while waiting. A client that stops reading SHALL hold no query
permit and no database connection, and one that goes away SHALL end the
stream's work at once. Every read and heartbeat of a stream SHALL hold one
of a bounded number of permits per process for all of its queries, so the
streams of a process together use at most that many connections at once.
While it is behind the
head (from the open until a read ends short of a full page, and again
after any full page, or after any step that did not begin from the
stream's idle wait, such as one that follows frames a client held up), it
SHALL send at most one page per catch-up tick: each read made while behind
SHALL release its frames one tick after the read began (when the client
had taken all but the last of the previous page's frames, before the read
waits for anything), or after the fewest whole ticks that reach its end
when it takes longer. The tick SHALL be
configured (`CAMPBOOKS_CHANGES__CATCH_UP_TICK_MS`, 50 ms by default)
between 50 ms and one second, checked at startup. A stream that has caught
up SHALL send the changes a wake-up from its idle wait brings as soon as it
reads them. The
server SHALL limit open streams per caller and answer 429 beyond the
limit, and SHALL limit how many streams a caller opens per configured
window (across processes), answering 429 `rate_limited` beyond it, whatever
happens in the workspace. That window SHALL be between one second and a
day, checked at startup.

#### Scenario: Many subscribers
- **WHEN** many clients subscribe to the same workspace and one change is recorded
- **THEN** every client receives it once

#### Scenario: A client that stops reading
- **WHEN** a client stops reading its stream while changes keep arriving
- **THEN** the other streams of the process keep reading and delivering

#### Scenario: Reconnecting in a loop
- **WHEN** a caller opens more streams than the configured number within the window
- **THEN** the next open answers 429 `rate_limited`, and other callers are not affected

#### Scenario: A window longer than a day
- **WHEN** `CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS` is 0 or 86 401
- **THEN** the server refuses to start, naming the setting

#### Scenario: Catching up, one page a tick
- **WHEN** a client resumes with several pages of changes it may see waiting, and each page's read takes less than a tick
- **THEN** it receives the catch-up's first page one tick after the open, and each next page one tick after it took the previous one, whenever in a tick it took it

#### Scenario: A catch-up tick out of range
- **WHEN** `CAMPBOOKS_CHANGES__CATCH_UP_TICK_MS` is 49 or 1 001
- **THEN** the server refuses to start, naming the setting

#### Scenario: A live change after catching up
- **WHEN** a stream has caught up and a change it may see is recorded
- **THEN** it sends that change as soon as it reads it, without waiting for a tick

#### Scenario: A client that stalls
- **WHEN** a caught-up client stops reading while changes it may not see are recorded, then a change it may see, and it reads again
- **THEN** it receives that change one tick after it took its last frame, however many hidden changes were recorded meanwhile

### Requirement: Access is asked again on every page
An open stream SHALL show a page only under the containers the caller may
see as listed after every change of who sees inside a container that was
numbered up to that page: it SHALL keep the list it made and list them
again before showing a page whose range holds such a change (a change of a
record type the container-visibility port names as changing access,
whether or not the caller may see it), and at least once a heartbeat
interval. When the list made at a heartbeat holds a container the previous
list lacked (a gain no change announced), it SHALL send `reset` with
reason `access_changed`. It SHALL resolve the caller's membership before
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

#### Scenario: Removed after more than a page
- **WHEN** more than a page of changes a member may see and then their removal are numbered together
- **THEN** the stream sends `revoked` and ends without showing any of those changes

#### Scenario: Woken by changes that do not touch access
- **WHEN** a stream is woken again and again by mail and renames
- **THEN** it lists the caller's containers once, not once per wake-up

### Requirement: Nothing about what the caller may not see
What a stream sends SHALL depend only on the changes its caller may see and
on time: every frame's event, SSE `id` and data, and when heartbeats are
sent, SHALL be the same whether or not changes the caller may not see are
recorded (in an inbox they are not in, private to another person, or
written for client tokens only). The stream SHALL read in pages of changes
the caller may see, filtered before the page is cut, so how its frames
group never depends on such changes, and while it catches up each page
SHALL leave one tick after its read began, so when it leaves does not
depend on them either as long as its read takes less than a tick. Resume
tokens SHALL be opaque: sealed by the server so a client can neither read
nor alter one, of one length, new on every frame, and bound to the
workspace and the caller. The stream SHALL NOT send the workspace's
sequence numbers or its head in any form. A client resuming SHALL receive
the same outcome (`ready` and the same visible changes in the same pages,
or the same `reset`) whether or not such changes were recorded.

#### Scenario: Mail in an inbox the member is not in
- **WHEN** a member follows the feed and the workspace's other inbox, which they are not in, receives mail
- **THEN** the member's stream (its frames, ids, tokens' shape and heartbeat times) is the same as in a workspace where that inbox received nothing

#### Scenario: Every kind of caller
- **WHEN** a member, an admin, a client acting for a member and a client acting for nobody each follow the feed while changes they may not see are recorded (another member's mail, draft or connection, a client's draft)
- **THEN** each one's stream matches its stream in a workspace without those changes, and resuming from any token it received replays the same frames

#### Scenario: Catching up past many hidden changes
- **WHEN** a member resumes after more changes they may not see than a page holds, interleaved with changes they may see
- **THEN** they receive the same frames, in the same pages, each page as long after they took the previous one, all before the next heartbeat, as in a workspace without the hidden changes

#### Scenario: Probing with made-up cursors
- **WHEN** a client resumes with a number, a token of another workspace or caller, or an altered token
- **THEN** it receives `reset` with reason `unknown_cursor`, whatever the workspace's head
