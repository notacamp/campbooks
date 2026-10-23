## MODIFIED Requirements

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

## ADDED Requirements

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
