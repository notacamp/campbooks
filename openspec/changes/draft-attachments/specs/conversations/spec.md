## ADDED Requirements

### Requirement: Attaching a file to a draft
The draft's author SHALL be able to add a file to it with
`conversations.upload_draft_attachment`. The request SHALL carry the raw
bytes as its body, and SHALL name the draft's `version` and
`content_token` as a save does. The file SHALL be stored as a blob the
draft holds and added to its attachments as `uploaded`. The answer SHALL be
the draft with its new version. The assistant SHALL NOT be able to call it.

#### Scenario: A PDF attached
- **WHEN** Sam uploads `lease-4b.pdf` (400 KB) to their reply draft, naming its version
- **THEN** the draft lists `lease-4b.pdf` as uploaded with its size, its version goes up by one, and sending it carries the file

#### Scenario: Another member's draft
- **WHEN** a second sender of the inbox uploads to Sam's draft
- **THEN** the response is 404 and nothing is stored

#### Scenario: A stale version
- **WHEN** an upload names a version older than the draft's
- **THEN** the response is 409 `draft_changed` and the draft is unchanged

### Requirement: Limits on attached files
An upload SHALL be refused with 400 `file_empty` when it has no bytes. It
SHALL be refused with 413 `attachments_too_large` when the draft's
attachments would pass 25 MiB in total, and with 422 `too_many_attachments`
past 20 files. A draft that is scheduled or sending SHALL answer 409
`draft_not_editable`.

#### Scenario: Over the total
- **WHEN** a draft holds 24 MiB of attachments and a 2 MiB file is uploaded
- **THEN** the response is 413 `attachments_too_large` and the draft still holds 24 MiB

### Requirement: Blocked file types and clean names
An upload whose cleaned file name ends in an extension that mail providers
block (such as `.exe`, `.js`, `.bat` or `.scr`, case-insensitive) SHALL be
refused with 400 `attachment_blocked`. A file name SHALL be cleaned before
it is stored: no path, no control characters or quotes, at most 255 bytes.
A name left empty SHALL be refused with 400 `filename_missing`. The media
type sent SHALL be kept only when it is a well-formed `type/subtype`, else
it SHALL be `application/octet-stream`.

#### Scenario: An executable
- **WHEN** Sam uploads `setup.EXE`
- **THEN** the response is 400 `attachment_blocked` and nothing is stored

#### Scenario: A path in the name
- **WHEN** a client uploads with the name `../../etc/passwd`
- **THEN** the file is stored as `passwd`

### Requirement: Removing an attached file
A draft save's attachment list SHALL keep or drop uploaded files exactly as
it does forwarded ones. A dropped uploaded file SHALL no longer be held by
the draft.

#### Scenario: Remove after attaching
- **WHEN** Sam attaches a file, then saves the draft without it
- **THEN** the draft lists no attachments, and the blob is no longer held by the draft
