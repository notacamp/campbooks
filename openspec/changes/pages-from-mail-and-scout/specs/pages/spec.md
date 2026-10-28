## ADDED Requirements

### Requirement: Attachments saved to a page
`pages.attach_from_message` SHALL make up to 20 of a message's attachments
files of a page, without copying their bytes, only when the caller may write
the page and the `AttachmentBlobs` port answers each attachment for the
caller. If any attachment is not answered, the call SHALL answer 404 and
attach nothing. `pages.detach_files` SHALL undo it for files no block still
names.

#### Scenario: Saving a thread's files
- **WHEN** Ana saves both attachments of "Kickoff agenda" to "Client onboarding"
- **THEN** the page holds two files with their names, types and sizes, and no new blob is stored

#### Scenario: An attachment from an inbox the caller cannot read
- **WHEN** Rui asks to attach a file from a conversation in Ana's inbox to a page he can write
- **THEN** the response is 404 and the page holds no new file

### Requirement: A page as text
`pages.read_text` SHALL answer a page the caller may read as Markdown, in
block order, with mentions and embeds as `[type:id]` references that are
never resolved, capped at 64 KiB and cut at a block boundary with
`truncated: true`.

#### Scenario: A page with a mention
- **WHEN** Ana reads as text a page that mentions the task "Send the welcome pack"
- **THEN** the text shows the mention as `[tasks.task:<id>]` and not the task's title

### Requirement: Appending a section
`pages.append` SHALL add Markdown of at most 16 KiB at the end of a page the
caller may write, parsed into schema blocks on the server (unsupported syntax
becomes plain text, links keep only `https`, `http` and `mailto`), checked
like any `pages.sync` update. It SHALL answer an `append_id`.
`pages.unappend` SHALL remove that append's blocks that nobody has edited
since, and SHALL say how many stayed.

#### Scenario: Undo after an edit
- **WHEN** Scout's section of three bullets was appended, Ana edited one bullet, and she undoes the append
- **THEN** two bullets are removed, the edited one stays, and the answer says one stayed
