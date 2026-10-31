## ADDED Requirements

### Requirement: The note asks whether a task from the thread is done
When an open task of the member that came from the conversation asks Done? and no
need is open, the "where things stand" note SHALL add "Your task “<title>” is
still open." and ask "Is it done?" with Done (1) and Not yet (2). While it asks,
it takes the place of a stream suggestion's choice.

#### Scenario: Ana opens the thread she replied in
- **WHEN** Ana opens the conversation "Send the signed lease" came from
- **THEN** the note says the task is still open and offers Done and Not yet
