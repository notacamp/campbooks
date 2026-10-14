# Tasks

## 1. Kernel and identity

- [x] 1.1 `ProviderGrant.issuer`; identity sets it on connect/reconnect and on adopting a held grant
- [x] 1.2 Granted scopes split on spaces and commas; a Zoho answer without `scope` grants what was asked
- [x] 1.3 Tests: Zoho sign-up holds the mailbox grant with its issuer

## 2. connectors: schema and engine

- [x] 2.1 Migration `connectors__zoho_mail`: provider `zoho`, `provider_server`, stream `mailbox_key`/`address`, index renamed to `messages`/`oversized` with `rfc_message_id`
- [x] 2.2 Provider-neutral errors and token grants; credentials refresh and revoke per provider
- [x] 2.3 Several mailbox streams per connection; `capability_granted` per mailbox; `StreamView.address`
- [x] 2.4 Outbound plan shared by both adapters; budget keys per Zoho login

## 3. connectors: Zoho adapter

- [x] 3.1 `zoho/` client: data centres, accounts, folders, listing, raw message, details, writes, send, attachments; HTML-decoded metadata
- [x] 3.2 Reader: backfill, head runs, sweeps, vanished-message look-ups, per-folder cursor
- [x] 3.3 Write-back: read, flag, archive, unarchive (unarchive or move)
- [x] 3.4 Send: reply call, Sent-folder look-up before an unclear retry, fetch and announce the sent copy
- [x] 3.5 Settings `CAMPBOOKS_ZOHO__*`, `CAMPBOOKS_CONNECTORS__ZOHO__*`; intake and start for `zoho`
- [x] 3.6 In-process fake Zoho and tests: data centres, mailboxes, backfill, head, archive, delete, write-back, send, idempotency, throttling, budget

## 4. Dev fake and recording

- [x] 4.1 `scripts/fake-zoho-mail.ts` served by `pnpm --dir clients fake-zoho` (demo mail only)
- [x] 4.2 Recording scenario `zoho-mail`; vhs clip of the adapter tests

## 5. Finish

- [x] 5.1 Contracts (`connection-account` provider `zoho`), OpenAPI regenerated, client types regenerated, events catalogue
- [x] 5.2 CHANGELOG; `.sqlx` regenerated; gates green; recording at 375px and desktop
