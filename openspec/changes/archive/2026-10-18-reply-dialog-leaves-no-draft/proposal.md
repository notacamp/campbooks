## Why

The review of #583 (campbooks-cloud#220) left follow-ups
(campbooks-cloud#257):

- After a failed send, Back to the choices cleared the pinned message but
  kept the saved draft. The next save for another pick could then
  overwrite it, or leave an orphan draft behind.
- Nothing tested that the older message's draft is discarded once the newer
  one is answered and sent.
- A comment still said the composer reopens the newest message's draft.
- The member's own sent mail arriving in the conversation showed "A newer
  message from <you>".

## What Changes

- Back moves a saved draft aside, as "Answer the newer one" does. The next
  save is a fresh draft, and the saved one (still editable) is discarded.
- The newer-message note is shown only for received mail, never for the
  member's own.
- Tests: the older draft is DELETEd after "Answer the newer one" then Send;
  the failed send's draft is DELETEd after Back, another pick, then Send; no
  note for the member's own mail.
- The stale comment is fixed.

## Impact

- Spec: home-ui (A reply answers the message its draft was written for).
- Code: `ReplyDialog` and its tests. The note's wording and look are
  unchanged, so no new recording.
