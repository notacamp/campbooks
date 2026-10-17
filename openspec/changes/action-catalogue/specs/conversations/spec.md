# conversations Specification

## ADDED Requirements

### Requirement: Each need's own actions
With `CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED` on (off by default), the
model pass SHALL ask, in the same `decide` call as the triage questions and
each in a slot of its own, one likelihood question per action of the
catalogue that no triage answer already gives: `action_remind`,
`action_open_attachment` (only for a message with an attachment),
`action_forward` and `action_paid_automatically`. It SHALL rate every action:
Reply from the `needs_reply` answer (never for a no-reply sender), Remind,
Open attachment (never without an attachment) and Forward from their own
questions, Report from the `suspicious` answer and It's safe from its
complement, Already paid from `action_paid_automatically` and Still to pay
from its complement; an answer without probabilities rates nothing. The
facts SHALL only rule an action out and nothing SHALL read what an email
says. The ratings SHALL be stored per message
(`conversations.action_likelihoods`, with whether each cleared
`CAMPBOOKS_CONVERSATIONS__ACTIONS__FLOOR_PERCENT`, default 70), never as
decisions, so the triage answers are recorded and acted on exactly as
without the catalogue. A need in a summary SHALL then carry `actions`, at
most three: on a `review` need raised by `suspicious`, Report and It's safe;
on a `pay` need, Already paid and Still to pay, then Open attachment, Remind
or Forward; on any other need, Reply, Open attachment, Remind or Forward.
A need's own answers SHALL always be offered, likelier first; the others
only when they cleared the floor, likelier first, a pair of the combo table
offered as one first among them when both did. A need with nothing rated or
nothing to offer SHALL carry no `actions`. With the setting off, the
`decide` request SHALL hold exactly the triage questions, nothing SHALL be
rated, and every need SHALL read exactly as before, with no `actions`.

#### Scenario: Off
- **WHEN** the catalogue is off and a message is decided
- **THEN** the request holds only the triage questions and the conversation's need has no `actions`

#### Scenario: On
- **WHEN** the catalogue is on and Marta's question is decided with `needs_reply` at 0.9 and Remind at 0.8
- **THEN** the same call carries the catalogue's questions, the triage answers stored are the same as with it off, and the Reply need offers Reply (90) first

#### Scenario: A warning
- **WHEN** a phishing message's need is shown with `suspicious` at 0.2
- **THEN** it offers It's safe, then Report, and nothing else

### Requirement: Labelling the owner's actions
`campbooks-server labelling export --set actions --workspace <id> --days <n>
--out <folder>` SHALL write, under the guards of the money export (operator
only, inside the workspace with row-level security, a new folder readable by
the operator only, a warning that it holds real mail), the newest inbound
message of each conversation that had a need, with a `labels.csv` holding one
empty column per action of the catalogue. `cargo xtask eval actions
--set-dir <folder> --floor <x>…` SHALL read the owner's marks (a row with any
action marked is labelled, a blank cell in it meaning no; a row left blank is
skipped), send only labelled items to the model, rate them as the model pass
does, print per action and floor the precision, recall and counts, and how
often each pair of actions is done together, and write the report into the
folder. No floor SHALL be set from emails an agent wrote.

#### Scenario: Scoring a floor
- **WHEN** the owner marked Reply on two emails and the model rates them 0.9 and 0.55
- **THEN** at floor 0.5 Reply scores 100% precision and recall, and at 0.7 its recall is 50%
