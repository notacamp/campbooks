## MODIFIED Requirements

### Requirement: Each need's own actions
With `CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED` on (off by default), the
model pass SHALL ask, in the same `decide` call as the triage questions and
each in a slot of its own, one likelihood question per action of the
catalogue that no triage answer already gives: `action_remind`,
`action_open_attachment` (only for a message with a file attached: an
image its body shows, such as a signature logo, is no attachment),
`action_forward` and `action_paid_automatically`. It SHALL rate every action:
Reply from the `needs_reply` answer (never for a no-reply sender), Remind,
Open attachment (never without a file attached) and Forward from their own
questions, Report from the `suspicious` answer and It's safe from its
complement, Already paid from `action_paid_automatically` and Still to pay
from its complement; an answer without probabilities rates nothing, and a catalogue answer
missing or not fitting its type rates nothing for that action and fails
nothing: the triage answers are stored and acted on exactly as if it had not
been asked. The
facts SHALL only rule an action out and nothing SHALL read what an email
says. The ratings SHALL be stored per message
(`conversations.action_likelihoods`), never as decisions, so the triage
answers are recorded and acted on exactly as without the catalogue. While
the setting is on, a need in `conversations.list` and `conversations.get`
SHALL carry `actions`, at most three, under the floor configured then
(`CAMPBOOKS_CONVERSATIONS__ACTIONS__FLOOR_PERCENT`, default 70): on a `review` need raised by `suspicious`, Report and It's safe;
on a `pay` need, Already paid and Still to pay, then Open attachment, Remind
or Forward; on any other need, Reply, Open attachment, Remind or Forward.
A need's own answers SHALL always be offered, likelier first; the others
only when they cleared the floor, likelier first, a pair of the combo table
offered as one first among them when both did; a pair SHALL never hold two
actions that ask a follow-up in a dialog (Reply, Remind, Forward), which the
table is checked for when it is built. A need with nothing rated or
nothing to offer SHALL carry no `actions`. With the setting off, the
`decide` request SHALL hold exactly the triage questions, nothing SHALL be
rated or read, and every need SHALL read exactly as before, with no
`actions`, even one rated while the setting was on (it is a kill switch).

#### Scenario: Off
- **WHEN** the catalogue is off and a message is decided
- **THEN** the request holds only the triage questions and the conversation's need has no `actions`

#### Scenario: On
- **WHEN** the catalogue is on and Marta's question is decided with `needs_reply` at 0.9 and Remind at 0.8
- **THEN** the same call carries the catalogue's questions, the triage answers stored are the same as with it off, and the Reply need offers Reply (90) first

#### Scenario: Switched off again
- **WHEN** the catalogue rated a need's message and is then switched off
- **THEN** the need reads as before, with no `actions`

#### Scenario: A warning
- **WHEN** a phishing message's need is shown with `suspicious` at 0.2
- **THEN** it offers It's safe, then Report, and nothing else

#### Scenario: A catalogue answer that does not fit
- **WHEN** the catalogue is on and the model leaves out `action_remind`, answers `action_forward` out of range and `action_paid_automatically` with "NaN"
- **THEN** the decision succeeds, its triage decisions and needs are the same as with the catalogue off, and Remind, Forward, Already paid and Still to pay are not rated

#### Scenario: Only a logo
- **WHEN** the catalogue is on and a message's only attachment is the signature logo its body shows
- **THEN** `action_open_attachment` is not asked and Open attachment is not rated
