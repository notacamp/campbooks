## MODIFIED Requirements

### Requirement: The Scout panel
The Scout panel SHALL open beside the page from `lg` up (the aside width,
the page keeping the rest) and over the whole screen below `lg`, and SHALL
show: the visor and "Scout" with Close (`Esc`); a line "About" with the
conversation's sender and subject and "Open" to open it; the chat's
messages, oldest first; and a field "Ask about this" with Ask (`Enter`,
`Shift+Enter` for a new line), under it the line "Scout asks before it changes
anything." Opening it SHALL call
`assistant.start_chat` for the conversation and show the chat that comes
back, so a chat already held continues. Closing it SHALL put the focus
back on what opened it. Opening it for another conversation SHALL show
that conversation's chat.

#### Scenario: Continuing a chat
- **WHEN** Sam asked about Marta's email yesterday and opens the panel on it again
- **THEN** the panel shows yesterday's questions and answers, and the field

#### Scenario: Closing on a phone
- **WHEN** Sam presses Close on a 375px screen
- **THEN** the panel closes and the focus is back on "Continue with Scout"

## ADDED Requirements

### Requirement: Change cards
Each change proposal SHALL show under Scout's answer as a card the client
writes from the action, the detail and the conversation's facts, in the
member's language ("Archive", "Move to Updates", "Later, until Monday
9:00", with "Marta Ferreira · Walkthrough on Thursday"), with Confirm and
Not now. A label name SHALL show as Scout's words, quoted. Confirm SHALL
send the proposal's request through the client's API with the member's
token and the proposal's id as the `Idempotency-Key`, only when its
operation is one of the client's own list of the same actions, its method
`POST` or `DELETE` and its path under the current workspace; otherwise the
card SHALL say Scout can't do this here. Then the card SHALL say it is
done with Undo, the lists it touches SHALL refresh, and the client SHALL
settle the proposal `done`; Undo SHALL send the undo request, with the fields its
`from_response` names copied from the change's answer,
(`<id>:undo` as the key) and settle `undone`. Not now SHALL settle
`declined`. A refused request SHALL settle `failed` and the card SHALL say
it could not be done, with the API's reason when it has one. A settled
card SHALL show its state, without buttons, when the chat is opened again.

#### Scenario: Confirm and Undo
- **WHEN** Sam presses Confirm on "Archive · Marta Ferreira · Walkthrough on Thursday", then Undo
- **THEN** the conversation is archived by Sam's own request, then back in the inbox, and the card shows it was undone

#### Scenario: A need that changed
- **WHEN** Marta wrote again before Sam pressed Confirm on "Done"
- **THEN** the API answers 409 `need_changed`, nothing changes, and the card says it could not be done

#### Scenario: A double press
- **WHEN** Sam presses Confirm twice quickly
- **THEN** one request acts and the card shows done once

### Requirement: Rule and reply cards
A rule proposal SHALL show its sentence as Scout's words, quoted, with
"Review the rule" and Not now; "Review the rule" SHALL open Teach Scout
with the sentence, which compiles it and asks for the rule's confirmation
as Teach Scout does, and the card SHALL settle `done` when the rule is
saved and `declined` when Teach Scout closes without saving. A reply
proposal SHALL show its intent as Scout's words, quoted, with "Write the
draft" and Not now; "Write the draft" SHALL open the conversation's Reply
dialog with that intent, through an opener in `lib/` the app provides, and
settle `done`. Nothing SHALL be sent or saved by a card.

#### Scenario: Teaching from the chat
- **WHEN** Sam asks "Can newsletters like this stay off Home?" and Scout proposes "Keep newsletters from The Weekly off Home"
- **THEN** "Review the rule" opens Teach Scout with that sentence and the rule is saved only on Teach Scout's own confirmation

#### Scenario: Drafting a reply
- **WHEN** Scout proposes the intent "Confirm Thursday at 10 and ask who else is coming"
- **THEN** "Write the draft" opens Marta's Reply dialog drafting that intent, and nothing is sent
