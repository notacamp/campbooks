## ADDED Requirements

### Requirement: A quote says when it is cut
`conversations.get` SHALL mark a message `quote_cut` when the `text` it
returns holds only the first part of the message's words: the text part was
cut, the HTML was cut at the 256 KiB it is read from, or the words were cut
at their limit. The message's `truncated` SHALL stay about its text part
alone, so HTML the reader shows whole is never called truncated.

#### Scenario: Long HTML read in time
- **WHEN** a member opens a conversation whose HTML-only message is 280 KiB and its words are read within the deadline
- **THEN** the message has its words, is not `text_unread` and not `truncated`, and is marked `quote_cut`; a short HTML-only message is not
