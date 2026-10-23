## ADDED Requirements

### Requirement: Ask Scout about a task
A task's detail SHALL offer "Ask Scout" in its header row, with the key
`.` shown on it, wherever a chat can be opened. It SHALL open Scout's chat
about that task, and the focus SHALL return to where it was asked from when
the chat closes. The chat SHALL name the task by its title and offer
starter questions about what is left, when to do it and who could take it.

#### Scenario: Asking about the lease
- **WHEN** Ana opens "Send the signed lease to Marta" and presses `.`
- **THEN** Scout's chat opens about that task, naming it and offering "What's left on this?", "When should I do this?" and "Who could take this?"
