---
title: "Email Scanning"
description: "How Campbooks scans and processes email messages"
sidebar_position: 2
---

Campbooks scans your connected email accounts and processes messages and attachments.

## How scanning works

1. **Scan Job**: A background job fetches messages from your email provider's API
2. **Deduplication**: Messages are matched by provider message ID to avoid duplicates
3. **Processing**: Each new message is processed to download attachments
4. **Document Creation**: Attachments become Documents with AI classification

## Manual scanning

Go to **Email Scans** and click "New Scan". Select the email account and folder to scan.

## Automatic scanning

Campbooks runs recurring scans on a schedule (configurable per email account). The default interval is every 5 minutes.

## Scan status

| Status | Meaning |
|--------|---------|
| Pending | Scan queued, waiting to run |
| Running | Scan in progress |
| Completed | Scan finished successfully |
| Failed | Scan encountered an error |

## Message processing

Each scanned message goes through a processing pipeline:

1. **Attachment download** — files are stored via Active Storage
2. **Document creation** — a Document record is created for each attachment
3. **AI classification** — the document type is determined by AI
4. **Indexing** — the document is indexed for full-text search

## Viewing messages

Scanned messages appear in the **Email Messages** section. You can:

- View the full message, including attachments
- Reply to the sender
- Add tags and labels
- See AI-generated action items and suggestions
