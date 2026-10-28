## ADDED Requirements

### Requirement: The marks a message may carry
A draft's text SHALL be plain text with marks, and only the server SHALL
turn it into HTML, escaping every character first. The marks SHALL be
`**bold**`, `*italic*`, `~~strikethrough~~`, `- ` or `* ` bullets, `1. `
numbered items, `> ` quotes, bare `http(s)://` links and `[words](url)`.

#### Scenario: A link with words
- **WHEN** the body is `See [the lease](https://example.com/4b).`
- **THEN** the HTML part has `<a href="https://example.com/4b">the lease</a>`

#### Scenario: Strikethrough
- **WHEN** the body is `rent on the ~~5th~~ 1st`
- **THEN** the HTML part shows `5th` struck through

### Requirement: Only safe addresses are linked
A `[words](url)` link SHALL be rendered only when its URL starts with
`http://`, `https://` or `mailto:` and holds no whitespace, `<`, `>` or
`"`. Any other SHALL stay as text.

#### Scenario: A link that is not to the web or to mail
- **WHEN** the body is `[click](javascript:alert(1))`
- **THEN** the HTML part shows it as text, with no `<a>`

### Requirement: The assistant writes no disguised links
Text that the member did not type as marks SHALL keep its `[` and `]` as text
when it becomes a draft's body, so no `[words](url)` in it becomes a link
whose words hide where it goes. This covers a draft created or saved for the
assistant rather than the member's own request, and Scout's draft
saved from the Reply or Nudge dialog. Bare addresses still link, showing
where they go.

#### Scenario: Scout is asked to hide a link
- **WHEN** the assistant creates a reply draft whose text is `Pay [here](https://example.net/pay) today.`
- **THEN** the draft's body keeps the brackets as text, and the sent HTML has no link worded "here"

### Requirement: Escaping a mark
A `\` SHALL make the next character plain text when that character is `*`,
`~`, `[`, `]` or `\`, or is a line's leading `-`, `>` or `1.`. A `\` before
anything else SHALL stay as written.

#### Scenario: An escaped mark
- **WHEN** the body is `2 \* 3`
- **THEN** the HTML part reads `2 * 3`, with no emphasis

#### Scenario: An old draft with a backslash
- **WHEN** a body saved before this change reads `C:\temp`
- **THEN** it renders as `C:\temp`

### Requirement: Fewer marks in the quoted history
Links with words, strikethrough and escapes SHALL apply only to the member's
own text. The quoted history SHALL be rendered on its own, with bare links,
emphasis, lists and quotes only, so text from someone else's mail never
becomes a link whose words hide where it goes.

#### Scenario: A worded link in the quote
- **WHEN** the quoted history holds `[your bank](https://example.net)`
- **THEN** the HTML part shows those characters as text, with `https://example.net` linked as a bare address and no link worded "your bank"

### Requirement: Marks in the plain-text part
The plain-text part SHALL write a link with words as `words <url>`, drop
the escapes, and keep the other marks as typed.

#### Scenario: A link in plain text
- **WHEN** the body is `See [the lease](https://example.com/4b).`
- **THEN** the plain-text part reads `See the lease <https://example.com/4b>.`

#### Scenario: An escape in plain text
- **WHEN** the body is `2 \* 3`
- **THEN** the plain-text part reads `2 * 3`
