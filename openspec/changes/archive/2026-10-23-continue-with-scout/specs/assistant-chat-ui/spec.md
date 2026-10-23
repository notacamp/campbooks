## ADDED Requirements

### Requirement: The Scout panel
The Scout panel SHALL open beside the page from `lg` up (the aside width,
the page keeping the rest) and over the whole screen below `lg`, and SHALL
show: the visor and "Scout" with Close (`Esc`); a line "About" with the
conversation's sender and subject and "Open" to open it; the chat's
messages, oldest first; and a field "Ask about this" with Ask (`Enter`,
`Shift+Enter` for a new line), under it the line "Scout looks things up and
explains. In this version it changes nothing." Opening it SHALL call
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
client from the conversation ("Ask me about <sender>'s email, <subject>.")
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
to open it for a conversation through `lib/`, without importing the
`assistant` module, and with no effect where no panel is mounted (tests,
stories). The command menu SHALL list nothing new in this phase.

#### Scenario: From Home and from the reader
- **WHEN** Sam opens the panel from a Home need, then opens the same conversation in the reader and presses `.`
- **THEN** both show the same chat

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
