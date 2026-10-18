## MODIFIED Requirements

### Requirement: Labelling the owner's money mail
`campbooks-server labelling export --workspace <id> --member <id or address>
[--inbox <id>] --days <n> --out <folder>` SHALL write, from the inboxes that
member can read (as the API decides it: never a teammate's private inbox;
only `--inbox` when given, refused when the member cannot read it), the
newest inbound message (not in spam or
the trash) of each conversation in Receipts or that had a `pay` need, sent
within the last `n` days (default 90), newest first and at most 500, as an
unlabelled eval set, readable by the operator only: `manifest.json` (a private set asking `money`),
`items/owner.jsonl` (what the decide model reads of each message: envelope,
list and automation headers, attachment names and media types, the start of
the text body; items alternating `dev` and `test`) and `labels.csv` (id,
date, sender, subject, attachments and an empty `money` column). It SHALL run
inside the workspace with row-level security, refuse a folder that exists,
refuse to run without `--member` (there is no whole-workspace export), say before reading any
mail the member id, role and inbox ids it resolved, mark the folder as its own
(`.campbooks-labelling`, recording that workspace, member, role, inboxes and
set), and print its absolute path,
that it holds real mail to keep off the repository, and to delete it from the
server once copied; a member who is suspended, of a workspace being deleted,
or who reads no inbox is refused. `campbooks-server labelling purge --out
<folder>` SHALL remove a folder only when it carries that marker and holds
nothing but what an export and its eval write, never following a link and
never any other folder. A committed eval set (under `crates/ai/evals/`) SHALL
never be `private`. `cargo
xtask eval decide --set-dir <folder>` SHALL read such a set where it is,
fill each item's `money` label from the sheet, leave unlabelled items out of
the scores, skip the fictional set's reserved-domain and id checks for a
private set (including its duplicate check), send only labelled items to
a model, accept a sheet saved with a byte-order mark or `;` separators, and
write its report into the folder.

#### Scenario: Exporting
- **WHEN** Sam's workspace holds a bill from Acme Hosting in Receipts, a question from Marta and a phishing message
- **THEN** the export lists only the bill, as `owner-001`, with empty labels

#### Scenario: Another workspace
- **WHEN** the export names another workspace
- **THEN** none of Sam's mail is written

#### Scenario: Labels from the sheet
- **WHEN** the owner writes `owed` in the `money` cell of one row and leaves another blank
- **THEN** the eval scores the first item against `owed` and leaves the second out

#### Scenario: A teammate's private inbox
- **WHEN** Sam and Leo each have a private inbox in one workspace and the export names Sam
- **THEN** only mail to Sam's inbox is written, whatever the set; naming Leo's inbox for Sam is refused

#### Scenario: No member
- **WHEN** the export names no `--member`
- **THEN** it is refused and nothing is written

#### Scenario: Purging
- **WHEN** the operator purges a folder without the marker
- **THEN** nothing is removed; a folder the export wrote is removed

#### Scenario: Saying whom it is for
- **WHEN** the operator exports for sam@example.com
- **THEN** before any mail is read it prints Sam's member id, role and inbox ids, and the marker records them
