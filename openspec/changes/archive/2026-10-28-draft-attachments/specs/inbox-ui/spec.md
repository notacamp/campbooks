## ADDED Requirements

### Requirement: Attaching files in the composer
The composer SHALL offer Attach (⇧⌘U), which opens the device's file picker
for several files. Files dragged onto the composer, or pasted into its text,
SHALL attach too; text pastes as before. Each file SHALL show "Uploading…"
until it is stored, then its name, size and a Remove button. Once anything
is attached, the composer SHALL show the total against 25 MB.

#### Scenario: Attach two files
- **WHEN** the user presses ⇧⌘U and picks two PDFs
- **THEN** both show as uploading, then with their names and sizes, and the total reads, for example, "1.2 MB of 25 MB"

#### Scenario: Drop a file
- **WHEN** the user drags a file over the composer
- **THEN** the composer shows "Drop to attach", and dropping the file attaches it

#### Scenario: Paste a screenshot
- **WHEN** the user pastes an image from the clipboard into the text
- **THEN** it is attached as a file and the text is unchanged

### Requirement: Files the composer refuses
Before uploading, the composer SHALL refuse a file that is empty, would take
the total past 25 MB, or has a blocked type. It SHALL say which file and
why in the composer, beside its files, and upload nothing for it. A refusal from the server SHALL
be said the same way.

#### Scenario: Too large
- **WHEN** 24 MB are attached and the user adds a 2 MB file
- **THEN** the composer says that file would take the message over 25 MB, and it is not attached

#### Scenario: A blocked type
- **WHEN** the user drops `setup.exe`
- **THEN** the composer says that file type can't be sent by email, and it is not attached
