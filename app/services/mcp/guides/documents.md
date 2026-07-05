# Documents

Campbooks extracts structured data from email attachments (PDFs, images) and
stores each one as a Document record with two independent status axes.

## Two-status axes

Every document has:

- **ai_status** — where the AI processing is: `pending` → `processing` →
  `complete` or `failed`. A failed document may need reprocessing or manual
  field entry.
- **review_status** — where the human review is: `pending` → `approved` or
  `rejected`. Reclassification resets this to pending and re-approves.

get_overview shows `documents.needs_review_count` (review_status = pending
and ai_status = complete) and `documents.ai_failed_count` (ai_status = failed).
Work through the review queue with list_documents filtered by
`review_status: "pending"`.

## Review flow

- **approve_document(id)** — marks the document approved and triggers final
  processing (e.g. Drive/Notion export if configured). Use when the extracted
  fields look correct.
- **reject_document(id)** — marks it rejected. The document stays but is
  excluded from reporting and approval counts.
- **reclassify_document(id, document_type_id)** — changes the type AND approves
  in one step. Use when the AI picked the wrong type.

Always show the user the extracted fields (get_document) before approving.
Do not bulk-approve without the user reviewing at least a sample.

## Extracted fields

get_document returns the full serialised document including any extracted fields:
vendor_name, vendor_nif, document_date, due_date, invoice_number, amount_cents,
currency, description, and more. update_document lets you correct them
without changing the review state.

Monetary amounts are stored in cents (integer). Currency is a 3-letter ISO code.

## Uploading documents

upload_document accepts a Base64-encoded file and queues AI processing. The
response comes back with `ai_status: "pending"` — poll get_document(id) or
list_documents(review_status: "pending") to see when it is ready.

## Filing into folders

file_document(mail_folder_id, document_id) adds the document to a folder.
A document can live in multiple folders. unfile_document(id) takes the
FolderMembership id (returned by file_document), not the document id.

## Searching documents

list_documents accepts `document_type_id` and `review_status` filters.
For text search across document content, use Scout (send_scout_message) —
the chat model has access to document content via its tools.
