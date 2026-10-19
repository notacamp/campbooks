# Asks that lapse or go stale

## Why
Home showed "<sender> asked you to do something" for an old sign-in-link email. The link expired ten minutes after it was sent, and the member had already used it; Scout's summary still said "expires in 10 minutes". Three things let that happen:

- A need is raised once and settled without a clock. A `do` or `reply` need carries no due date (campbooks-cloud#146), so the one expiry Home knows (an item leaves 7 days after its due date) never applies, and the needs window only gates raising: an open need stays until a member clears it.
- Nothing ever works out how long an ask stays possible.
- Scout's summary is written once and kept, but its wording counts down from the moment it was written.

Modules: `conversations` and `attention` (server), `conversations` and `attention` (client). Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/275. Folds in https://github.com/notacamp/campbooks-cloud/issues/146.

## What changes
- **Dates from Scout's read.** The `complete` call that already summarises a need's email when the need opens also returns `lapses_at` (when what is asked stops being possible: a link or code expiring, an offer or an RSVP closing) and `due_on` (the date by which the recipient is asked to act), worked out from the email's words and its sent time, held to the strict schema. Rust drops a date that does not parse, comes before the message was sent, or lies more than a year after it. No content rule reads the mail (ADR 0018). Both are kept with the message and carried on the need, on `conversations.need_changed` and on the attention item.
- **Lapsed asks leave Home.** A `do`, `reply` or question need whose lapse time has passed is cleared with a new cause, `lapsed`. Pay needs never lapse (a bill stays until answered) and phishing warnings keep their own rules. `attention.list` also hides an item at read time once its lapse time has passed, so a ten-minute link leaves Home without waiting for anything. In the inbox and the reader the conversation shows an "Expired" chip.
- **Old asks become "Still needed?".** A `do` or `reply` need whose message was sent longer ago than the needs window (14 days), with no due date and no lapse time ahead, that no member confirmed, becomes a Review question: "Pedro Alves asked 3 weeks ago. Still to do?" or "Pedro Alves wrote 3 weeks ago. Still needs a reply?". Yes keeps it as the member's own (never asked again); Not needed clears it. Both undo. Neither stores a correction: the model read the mail right, time passed.
- **A clock settles needs.** A need is settled against the time now. A table of when each open need next changes on its own (`conversations.need_clocks`, ids and times) is read every minute by a dispatcher, which wakes each workspace's due needs.
- **Summaries don't count down.** The instruction forbids time relative to now ("in 10 minutes", "today", "expires soon") and asks for times anchored to the sending.
- **Bills keep their dates.** Now that bills get due dates, a Pay item is no longer dropped from Home 7 days after its due date.
- **Open needs today.** On upgrade every inbox is queued for the needs backfill: each open need is settled against the clock (old asks become questions at once), and each open need whose email was summarised before dates existed is read once more, within the workspace's daily AI limit and only where the `complete` role may read mail.

## Non-goals
- Seeing whether a link was clicked or a code used: Campbooks cannot, so timing is the signal.
- Asking the `decide` model about dates (its questions are fixed and typed).
- Bringing back an expired ask by hand (recorded in `openspec/decisions-needed.md`).
- Reminders or notifications at the lapse time.
- Changing the needs window or making it per member.

## Impact
- Server: two migrations in `conversations` (columns on `needs` and `need_summaries`, the `need_clocks` table with a maintenance read policy, the widened checks validated in a second migration, the backfill queued), one in `attention` (`items.lapses_at`). Two new jobs (`conversations.dispatch_need_clocks`, a schedule, and `conversations.wake_needs`), a new `architecture.toml` maintenance-scope entry. `conversations.need_changed` v1 gains an optional field (additive; old events replay). `NeedView` gains `sent_at` and `lapses_at`; `ConversationSummary` gains `lapsed_need`; `NeedQuestion` gains `still_to_do` and `still_needs_reply`.
- Client: the still-needed sentences and toast, the Expired chip, in four languages.
- No new scope or endpoint.
