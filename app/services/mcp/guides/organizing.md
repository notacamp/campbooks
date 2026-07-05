# Organising emails and documents

Campbooks has three independent classification axes: **tags**, **categories**,
and **folders**. They serve different purposes and are not interchangeable.

## Tags

Tags are user-defined labels that apply to emails. They are workspace-scoped
and appear in filter dropdowns. Create them with create_tag (name + optional
hex color). Apply to individual emails with add_email_tag or to a batch with
tag_emails (the tag must exist first). Remove with remove_email_tag.

Use tags for things you want to find again: "invoice-query", "client-acme",
"needs-follow-up". Categories are AI-assigned and immutable; tags are yours.

## Categories

Categories (personal/important/notifications/promotions/social/updates/unknown)
are assigned automatically by the AI classifier. You cannot create or rename
them. You can filter search_emails by `category`. To retrain the classifier's
behaviour for a sender or cluster, use the Skim learning loop (skim_decide
builds a decision memory the next triage pass reads).

## Folders

Folders are workspace-level containers that mirror provider mailbox folders.
Create one with create_folder. When `provision: true`, the folder is
created on every connected mailbox the user manages — the response includes a
`provision.created_count` and `provision.failed_count`. Provider folder creation
may fail for accounts the user can read but not manage; report failures to the
user rather than silently ignoring them.

Move emails (and their full threads) into a folder with move_emails_to_folder.
Passing `folder_name` works cross-account: if the provider folder does not
exist yet, it is provisioned on the fly. Passing `folder_id` targets a single
provider folder by its id.

Folder depth is capped at 3 levels. Attempts to create a deeper hierarchy
return a validation error.

Documents can be filed into folders independently (file_document /
unfile_document). A document can appear in multiple folders.

## Document types

Document types classify attachments (invoices, receipts, contracts, etc.).
They are workspace-scoped and drive AI extraction: once a type exists,
new documents matching it will have their fields filled automatically.
Create them with create_document_type (name + optional category from the
DocumentType::CATEGORIES list). Uniqueness is per workspace; duplicates
return a ToolError.

## Recommended setup order

When setting up a new workspace (get_setup_status.next_steps will suggest this):
1. Create document types for the attachments you receive most.
2. Create a few tags for recurring email topics.
3. Create top-level folders that mirror your filing convention, then provision
   them so every connected mailbox has the matching provider folder.

Once taxonomy is in place, incoming mail will be classified, documents will
be extracted, and you can use tag_emails / move_emails_to_folder to
route things without touching the web UI.
