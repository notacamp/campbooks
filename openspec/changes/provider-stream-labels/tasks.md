# Tasks

## 1. Design

- [x] 1.1 Proposal, design (alternatives: marks for every stream vs native first, folders vs labels at Zoho; loop safety; backfill guard), spec deltas

## 2. Connectors

- [x] 2.1 Verify Zoho's labels API and scope name against Zoho's Mail API docs; add `ZohoMail.tags.ALL` to the connect scopes
- [x] 2.2 `domain/marks.rs`: mark names, recognition by name, the stream ↔ place table per provider
- [x] 2.3 Gmail client `labels.create`; push creates a missing mark, refreshes `gmail_labels`
- [x] 2.4 `outbound::map`: Receipts via `CATEGORY_UPDATES` + mark, every move takes other marks off; marks never written by label changes
- [x] 2.5 Push: `already_set` skip when the change is already in place (Gmail and Zoho)
- [x] 2.6 Gmail read: marks out of `labels`, `provider_stream`, `stream_mark_stale` on a category change that keeps a mark
- [x] 2.7 Migration `connectors__zoho_labels` (RLS, grant); Zoho client list/create labels; label tokens `label:<id>` on read, write (`applyLabel` / `removeLabel`) and `after()`; marks → `provider_stream`
- [x] 2.8 `map_zoho`: labels and stream moves through marks; `scope_missing` for an old grant
- [x] 2.9 A write that changes a provider stream is reported once from the index (Gmail and Zoho)
- [x] 2.10 Zoho labels read through the listing's `labelid` filter: marks each run, every label at the end of a pass; label tokens never `already_set`
- [x] 2.11 Backfill guard: mail first indexed after the first sync but received before it completed (cursor `synced_at`) goes in a `backfill` batch

## 3. Conversations

- [x] 3.1 Contract `message-change-batch.v1`: optional `provider_stream`, `stream_mark_stale`; ingest types
- [x] 3.2 Migration `conversations__message_provider_stream`; stored on insert and update
- [x] 3.3 Provider move in `apply_changes` (move with source `member`, requests for other messages and stale marks)
- [x] 3.4 Mirror helper; called from `decide_message` placement and on a message joining a sorted conversation; backfill guard
- [x] 3.5 Mirror each conversation once per batch; validate migration for the provider stream check

## 4. Tests (fake Google, fake Zoho, in-process)

- [x] 4.1 Every scenario of both spec deltas
- [x] 4.2 Loop safety: a mirrored write, then the next sync, publishes no event and records no change
- [x] 4.3 Contract: batches without `provider_stream` change nothing

## 5. Finish

- [ ] 5.1 `.sqlx` regenerated, OpenAPI and events catalogue regenerated if they change
- [ ] 5.2 Changelog entry file; terminal recording (vhs) against the server with the fake providers
- [ ] 5.3 CI green; `openspec archive provider-stream-labels --yes` before merge
