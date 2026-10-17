# Tasks

## 1. Kernel

- [x] 1.1 `CAMPBOOKS_CHANGES__CATCH_UP_TICK_MS` (default 50, 50 to 1000, checked at startup); `.env.example`
- [x] 1.2 `Pacer`: a read made while behind (the first after the open, or after a full page) releases its frames one tick after it began (taken before its permit), or the fewest whole ticks that reach its end; unit tests on a paused clock (the client taking each page at any phase of a tick, tiny and large hidden volumes a tick after the start, a read longer than a tick in whole ticks, pacing again after a live full page)
- [x] 1.3 Containers listed once per stream (`Containers`, the head read before the list as `covers`), under the query permit with the page; listed again at every heartbeat
- [x] 1.4 `access_changes`: whether a membership or container-access record type was numbered in the page's range (after what was already accounted for), seen or not; a container change lists again and reads the page anew, a membership change resolves membership before the page is shown
- [x] 1.5 Stream and visibility-port docs
- [x] 1.6 Each stream runs in a task of its own, sending frames over a channel of one; the body only receives and holds the stream slot; every read and heartbeat holds one query permit for all of its queries

## 2. Conversations

- [x] 2.1 `create_inbox` publishes `conversations.inbox_changed` when it adds the member; test

## 3. Server

- [x] 3.1 Tests: the list kept across wake-ups and listed again when a member is taken out of an inbox (no id of its new mail) and when an inbox is made for them (its mail arrives at once); a removal the caller cannot see ends the stream before the heartbeat; catch-up pages on the tick in the busy and the quiet workspace
- [x] 3.2 `streams_left_unread_mid_read_hold_nothing_back`: eight streams read briefly and left again and again on a fast poll; a new stream still reads at once (the flaky many-subscribers test was fixed in the test by #532; this makes it safe by construction)

## 4. Security review

- [x] 4.1 Pacing counts from when the read began (before its permit), not from the open: the client can no longer shift the tick's phase to time the scan
- [x] 4.2 A container gained with no announcement, found at a heartbeat, sends `reset` (`access_changed`); server test
- [x] 4.3 Membership changes looked for up to the head the read saw, past a full page; server test
