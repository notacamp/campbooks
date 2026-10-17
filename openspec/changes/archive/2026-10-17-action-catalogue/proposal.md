# Each item's actions, rated from a catalogue (backend)

Part of campbooks-cloud#164; issue campbooks-cloud#166. The approved
prototype (Home quick actions v4) gives each item on Home its own actions,
picked from what Campbooks can actually do, ranked by the decide model, with
pairs usually done together offered as one. This change is the backend half:
the catalogue, the model's ratings, each need's actions on the API, and the
tooling the owner needs to set the floor. Home renders them in a second PR.

## What changes

- **The catalogue** (`campbooks_ai::decide::signals::actions`): Reply,
  Remind, Open attachment, Forward, Report, It's safe, Already paid, Still to
  pay. No calendar action until scheduling lands (#42).
- **One likelihood question per action** that no triage answer already
  gives: `action_remind`, `action_open_attachment` (only when the message
  has an attachment), `action_forward`, `action_paid_automatically`. Reply is
  the `needs_reply` answer, Report and It's safe the `suspicious` answer and
  its complement, Already paid and Still to pay `action_paid_automatically`
  and its complement. They are asked in the same `decide` call as triage,
  each in a slot of its own; the facts only rule an action out (no
  attachment, a no-reply sender), never add one (ADR 0018). Nothing reads
  what an email says.
- **Ratings stored per message** (`conversations.action_likelihoods`), never
  as decisions: the triage answers are recorded exactly as before.
- **Each need's actions** (`NeedView.actions`, at most three): the need's own
  answers first (Report / It's safe on a warning, Already paid / Still to
  pay on a bill, always offered, likelier first), then the other actions its
  kind allows that cleared the floor, likelier first, a combo first among
  them.
- **Off by default** (`CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED`): the
  decide request and every need are byte-for-byte as before until the owner
  has labelled an eval and the floor
  (`CAMPBOOKS_CONVERSATIONS__ACTIONS__FLOOR_PERCENT`, a placeholder 70) is set
  from it.
- **Owner tooling**: `campbooks-server labelling export --set actions` (mail
  that had a need, one column per action) and `cargo xtask eval actions
  --set-dir <folder> --floor <x>` (precision and recall per action at each
  floor, and how often pairs are done together, from the labels).
- **Fix**: the OpenAI-compatible `decide` path sends `max_tokens` 4096, so a
  reasoning model is never cut off before its JSON.

## Out of scope

Home's rendering (the second PR), calendar actions (#42), reading invoices,
and any value of the floor: the owner's labels set it.
