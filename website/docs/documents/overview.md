---
title: "Document Overview"
description: "How document management works in Campbooks"
sidebar_position: 1
---

Documents are at the heart of Campbooks. Every email attachment becomes a document that's classified, analyzed, and tracked through your review workflow.

## Document lifecycle

1. **Ingested** — an email attachment is received and a document is created
2. **Processing** — AI analyzes the document to determine type and extract data
3. **Ready for Review** — the document appears in your dashboard, ready to review
4. **Approved / Rejected** — you approve or reject the document
5. **Exported** — optionally, push the document to Google Drive, Zoho WorkDrive, or Notion

## Document types

Campbooks can recognize these document types (and you can add custom ones):

- Invoices
- Receipts
- Contracts
- Statements
- Tax documents
- Reports
- Forms
- Correspondence

AI classification uses Claude Vision to analyze each document and suggest the appropriate type.

## Document statuses

| Status | Meaning |
|--------|---------|
| Pending | Just ingested, waiting for processing |
| Processing | AI analysis in progress |
| Needs Review | Ready for human review |
| Approved | Reviewed and approved |
| Rejected | Reviewed and rejected |
| Failed | Processing encountered an error |

## Search and filter

Documents are searchable by:

- Full text (via OpenSearch or PostgreSQL)
- Document type
- Status
- Date range
- Source email account
- Tags
