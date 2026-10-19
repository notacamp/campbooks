## ADDED Requirements

### Requirement: AI calls are timed in the logs
A streamed AI call SHALL write one log line when it ends, with its outcome
(`done`, `failed` or `cancelled`), the time to its first token and the time
it took in milliseconds, the characters of reasoning and text received, why
the model stopped (a fixed word: `stop`, `length`, `content_filter`,
`tool_calls`, `other` or `none`) and the prompt and completion token counts.
A reply written or streamed for a member SHALL also log, once, how it ended
and why (`idle`, `total`, `empty`, `truncated`, `provider`), its time to
first token, its duration and its token counts. No such line SHALL hold a
prompt, a delta, the thinking, the reply or any other content.

#### Scenario: A streamed reply ends
- **WHEN** a streamed reply ends normally after 12 seconds
- **THEN** one line says `done` with its time to first token, `elapsed_ms` about 12000 and its token counts, and no field holds the reply's words

#### Scenario: A reply that went quiet
- **WHEN** a streamed reply ends because the model sent nothing for 30 seconds
- **THEN** one line says `timed_out` with the reason `idle`, and no content
