---
title: "Overview"
description: "What Campbooks is and how it works"
sidebar_position: 1
---

Campbooks is a source-available, AI-native email client for professionals and small businesses. It reads your email and documents, uses AI to file and surface what matters, and gives you a clear review-and-approval workflow — reimagined so it feels nothing like the email you're used to.

## What Campbooks does

- **Ingests emails** from Zoho Mail, Google Workspace, or Microsoft 365 via OAuth
- **Classifies documents** using AI — invoices, contracts, receipts, and more
- **Prioritizes action items** — knows what needs your attention right now
- **Manages approvals** — review, approve, or reject documents
- **Exports to your tools** — Google Drive, Zoho WorkDrive, Notion

## How it works

1. **Connect an email account** via OAuth. Campbooks scans your inbox for emails with attachments.
2. **AI classifies** each attachment — recognizing document types like invoices, contracts, receipts, and more.
3. **Documents appear in your dashboard** with statuses and action items. You can review, approve, or export them.
4. **Email integration** means you can reply, tag, and organize emails directly from Campbooks.

## Architecture

Campbooks is a Ruby on Rails application with:

- **PostgreSQL** for the database
- **Solid Queue** for background job processing
- **Tailwind CSS** for the interface
- **Hotwire** for interactive features
- **Claude (Anthropic)** for AI document analysis and classification

## Next steps

- [Install Campbooks](/docs/getting-started/installation) on your server
- [Connect an email account](/docs/email/connecting-accounts)
- [Configure AI services](/docs/ai/configuration)
