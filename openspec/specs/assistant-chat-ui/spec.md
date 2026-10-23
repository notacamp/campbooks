# assistant-chat-ui Specification

## Purpose
The Scout panel in the clients: opened from a Home need or the reader on `.`, it shows the chat about that conversation, streams Scout's thinking, steps and answer, lists what Scout looked at, and says plainly when Scout can't answer.

## Requirements

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

### Requirement: Questions to start from
A chat with no messages SHALL show Scout's opening line, built by the
client from what the chat is about (for a conversation "Ask me about
<sender>'s email, <subject>.")
and up to three questions to tap: "Why is this here?", "What happened to
this email?" and "What else has <first name> sent?" (the last only when
the sender has a name). Tapping one SHALL ask it. They SHALL be gone once
the chat has a question. No model SHALL be called to write them.

#### Scenario: A first question
- **WHEN** Sam taps "Why is this here?"
- **THEN** it is asked as if he had typed it, and the questions to start from are gone

### Requirement: Scout writing
While Scout answers, the panel SHALL show the visor thinking, Scout's
thinking as it comes (in the `ThinkingTrace`, folded once the answer
starts), a line for each step as it runs, and the answer's text as it is
written; on the desktop, where answers don't stream, the visor thinking
until the whole message comes. The answer SHALL be shown as plain text:
paragraphs, and lines starting with `- ` as a list; never as HTML, never
turned into links. Each Scout message with steps SHALL offer "What I
looked at", a disclosure listing each step as a sentence in the member's
language built from the tool and its count ("Read what happened to this
email", "Looked for conversations: 3 found"), with each conversation it
returned opening on activation. The field SHALL stay usable while Scout
writes, and Ask SHALL wait until the answer is done. Closing the panel
while Scout writes SHALL stop the request.

#### Scenario: Steps as they run
- **WHEN** Scout reads the history and then looks for Marta's conversations
- **THEN** the panel shows "Reading what happened to this email…", then "Looking for conversations…", then the answer, with "What I looked at" listing both

#### Scenario: Markup in an answer
- **WHEN** an answer holds `<b>Pay</b>` and `https://example.com`
- **THEN** both show as the plain characters, with no bold and no link

### Requirement: When Scout can't answer
The panel SHALL say, in place of the field, that Scout can't chat on this
server for `chat_unavailable`; that the workspace used today's AI for
`ai_daily_limit_reached`; that Sam asked a lot this hour for 429, keeping
his words in the field; and for `chat_full` that the chat is full, with
"Start over", which deletes the chat (`assistant.delete_chat`) and starts
a new one. A failed answer (`chat_failed`, a stream that ends without
`done`, a stored message with finish `failed`) SHALL say "Scout couldn't
answer that." with Try again, which asks the same question again. An
answer cut short SHALL say so under it.

#### Scenario: Try again
- **WHEN** Scout's answer fails and Sam presses Try again
- **THEN** the same question is asked again and the failure line is gone

### Requirement: Opening the panel from anywhere
The app SHALL mount one Scout panel per workspace and give modules a way
to open it through `lib/` for a record of a registered type, or for
nothing with a first question, without importing the `assistant` module,
and with no effect where no panel is mounted (tests, stories). It SHALL be
offered as "Ask Scout" in Scout noticed's actions (about the suggestion),
in Teach Scout after a rule is saved and in each rule's overflow in the
rules list (about the rule), in the Reply and Nudge dialogs and on each
Waiting on others row (about the conversation), and in the command menu
(⌘K): "Ask Scout" opens a new chat about nothing, and with text typed in
the menu, "Ask Scout: <text>" opens one and asks it. Opening it from a
dialog SHALL leave the dialog as it was when the panel closes.

#### Scenario: From Home and from the reader
- **WHEN** Sam opens the panel from a Home need, then opens the same conversation in the reader and presses `.`
- **THEN** both show the same chat

#### Scenario: From the command menu
- **WHEN** Sam presses ⌘K, types "what am I waiting on" and chooses "Ask Scout: what am I waiting on"
- **THEN** the panel opens on a new chat about nothing and that question is asked

#### Scenario: From a Scout noticed suggestion
- **WHEN** Sam presses "Ask Scout" on "builds@ci.example.com: keep them off Home?"
- **THEN** the panel opens on a chat about that suggestion, offering "Why do you suggest this?"

### Requirement: Accessible, localized, phone-ready
The panel SHALL exist in en, pt-PT, es-ES and fr, use only `lib/ui`
components and tokens, be a labelled region (desktop) or a dialog (below
`lg`) that traps focus, announce Scout's finished answer in a polite live
region, work at 375px with no horizontal scroll, show its shortcuts from
the registry (`Esc`, `Enter`, `.`) and pass the axe checks in light and
dark.

#### Scenario: French at 375px
- **WHEN** a French member opens the panel on a 375px screen
- **THEN** every label reads in French and nothing scrolls sideways

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

### Requirement: Starter questions per kind
The questions to start from SHALL depend on what the chat is about: for a
rule "What does this rule do?" and "What has it changed?"; for one of
Scout's suggestions "Why do you suggest this?" and "What would it
change?"; for nothing "What needs me today?" and "What am I waiting on?";
for a conversation, as before. The about line SHALL show a rule as the
client writes its sentence, a suggestion by its sender, and nothing as
"About your mail".

#### Scenario: A chat about a rule
- **WHEN** Sam opens Scout from a rule he just saved
- **THEN** the panel offers "What does this rule do?" and "What has it changed?"

### Requirement: Recent chats
The panel's header SHALL offer "Recent", listing the member's chats
(`assistant.list_chats`) by their first question and the time of their
last message; choosing one SHALL open it. A chat with no question yet SHALL
not be listed. The list SHALL be reachable by keyboard and SHALL fit at
375px.

#### Scenario: Back to yesterday's chat
- **WHEN** Sam opens Recent and chooses "Why is this here?" from yesterday
- **THEN** the panel shows that chat with its messages
