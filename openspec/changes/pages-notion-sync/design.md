# Design

## Owner decisions (2026-10-25)

- **Direction:** import, then two-way sync.
- **Scope:** only the pages the member shares in Notion's consent screen.
- **Who:** each member connects their own Notion account, and imported pages
  start in their Private place.

## Alternatives considered

**Where the Notion OAuth flow lives.**

- *Owned by connectors* (chosen), like `connect_imap`. Notion's OAuth returns
  no ID token, while the identity front door verifies one (`iss`, `aud`,
  nonce) and makes it a way in. Notion is never a way in, so connectors runs
  the authorization-code flow itself:
  - `state` bound to the browser, single use, 10 minutes;
  - PKCE sent as well, although Notion ignores it;
  - the result goes to the existing `grants::create_or_refresh_connection`
    seam.
- *A new identity purpose that skips the ID token.* Rejected: it widens the
  sign-in code for something that is not sign-in.

**What a sync moves.**

- *Blocks, keyed by Notion block id* (chosen). Notion gives a stable id and
  `last_edited_time` for every block. Pages gives every block a stable `bid`
  attribute (added to the block schema), and a mapping table pairs them. Only
  changed blocks move, and a Campbooks edit to another block is never
  overwritten.
- *Replacing the whole page on every change.* Simple, but each Notion edit
  would wipe concurrent Campbooks edits. Rejected.
- *Character-level merge with Notion.* Impossible: Notion's API reads and
  writes a block's whole rich text.

**When both sides changed the same block.**

- *Keep both and ask* (chosen). Campbooks keeps its version in place and adds
  a `conflict` block holding Notion's version, with Keep mine, Use Notion's
  and Keep both. That block isn't sent until the member chooses.
- *Last writer wins.* Rejected: it silently loses someone's words.

**How Campbooks hears about Notion changes.**

- *A signed webhook first, polling as the fallback* (chosen). Notion's
  `page.content_updated` is a signal only (ids and a time, about a minute
  late). It makes the page's stream due, and the run reads the blocks.
  Without a webhook secret (self-hosters, dev), each connection polls Notion
  search for pages edited since its cursor every 5 minutes, adaptive within
  `CAMPBOOKS_CONNECTORS__POLL__*`.

## Connectors: the `notion` provider

- **Migration:** widen `connectors.connections.provider` with `notion` and
  `capabilities` with `pages`.
- **Connection identity:**
  - `account_subject` = Notion's `bot_id`, the per-authorization id;
  - `issuer` = `https://api.notion.com`;
  - the display name = Notion `workspace_name`.
  Unique per (workspace, provider, `bot_id`).
- **OAuth:**
  - `POST /api/v1/w/{ws}/connections/notion/authorizations` (a member only;
    `connections:write`) answers the Notion authorize URL;
  - `GET /auth/callback/notion` exchanges the code with HTTP Basic client
    auth;
  - the client comes from `CAMPBOOKS_NOTION__CLIENT_ID` and
    `CLIENT_SECRET[_FILE]`, with `REDIRECT_PATH` defaulting to
    `/auth/callback/notion`.
  Tokens are sealed with the connectors `Cipher` purpose and AAD
  `connectors.credentials:{column}:{connection_id}`. Each refresh stores the
  new access and refresh pair in one transaction (rotation). An
  `invalid_grant` marks the connection `needs_reconnect`.
- **Client:** follows the Microsoft client.
  - Fixed host `api.notion.com`, with `Notion-Version` pinned.
  - Ids are validated UUID path segments, and nothing in a response builds a
    URL.
  - No redirects, a 15 s call timeout, and answers capped at 1 MiB per page of
    block children.
  - Retry-After capped at an hour, and errors carry status and code only.
  - Calls are charged to the token bucket `notion:{connection}`: three per
    second, and 120 per minute per connection.
- **Change stream** `notion.pages`, one per linked Notion page:
  - The cursor is the newest `last_edited_time` seen.
  - A run lists the page's block tree (paged, at most 2,000 blocks and 12
    levels deep) and emits a `NotionPageSnapshot`. That is a normalized,
    provider-neutral block list: Notion id, kind, attributes, rich-text runs,
    `last_edited_time` and a content hash, and an `external` kind with a
    label for kinds Pages lacks.
  - Notion files are downloaded (Notion's URLs expire in an hour), capped at
    20 MiB with a content-type allowlist, stored as blobs, and referenced by
    hash.
- **Webhook** `POST /hooks/notion`: public, outside the operation catalogue,
  and absent unless `CAMPBOOKS_NOTION__WEBHOOK_SECRET[_FILE]` is set.
  - It answers the one-time verification post and logs that a token arrived,
    never its value. The operator copies the token from Notion's dashboard
    into the secret.
  - Every other post is checked in constant time against
    `X-Notion-Signature`, an HMAC-SHA256 of the raw body.
  - Body cap 64 KiB, with the same per-address and instance limits as the
    Microsoft hook.
  - For `page.content_updated`, `page.deleted` and `page.moved`, it looks up
    streams by Notion page id in the maintenance scope and makes them due.
    It never calls Notion and never trusts more than the ids.
  - Unknown posts get 404.
- **Outbound:**
  - Pages records outbound changes of kind `notion.block_update`,
    `notion.block_append` and `notion.block_delete`, MAC-bound like mail
    outbound changes (workspace, stream, Notion page, block, kind).
  - The run verifies the MAC before it calls Notion.
  - A block whose Notion `last_edited_time` moved since the recorded base is
    not written. It comes back as a conflict on the next inbound run.
  - Deletes only remove blocks that Campbooks itself mapped and the member
    deleted. A page is never archived in Notion.

## Pages: linking and applying

- **Block ids:** every block element gets a `bid` (uuid v7) attribute,
  allowlisted and assigned on creation by the client and by server inserts.
  Existing documents get theirs lazily on the next server-side apply.
- **`pages.external_links`:** `page_id`, `connection_id`, `notion_page_id`,
  `state` (`syncing` | `paused` | `stopped`), `stopped_reason`,
  `last_inbound_at`, `last_outbound_at`, `created_by`. One link per page, and
  one page per (connection, Notion page).
- **`pages.external_blocks`:** `page_id`, `bid`, `notion_block_id`,
  `base_hash` (the hash agreed at the last sync), `base_edited_at`. This is
  the three-way base: ours now, theirs now, and the base.
- **Import** (`pages.import_from_notion`, `pages:write`): the member picks
  Notion pages their own connection can see. Each becomes a private page
  under the chosen parent, sub-pages nested, linked and filled by the first
  inbound run.
- **Applying a snapshot** (a consumer of `notion.pages`, in one transaction):
  1. Map each Notion block to Campbooks blocks through a table, kind to kind:
     - paragraph → paragraph;
     - `heading_1` to `heading_3` → heading 1–3;
     - bulleted and numbered list items → list items;
     - `to_do` → task item;
     - toggle, quote, callout, divider, code, image and file, and
       `child_page` → the same kind (`child_page` as a page link).
     Anything else → `external`.
  2. For each block: if theirs changed and ours didn't, apply theirs; if
     ours changed and theirs didn't, the outbound path owns it; if both
     changed, insert a `conflict` block after ours; if neither, skip. Notion
     blocks with no mapping are inserted at their Notion position, and mapped
     blocks gone from Notion are removed unless ours changed (that also
     becomes a conflict).
  3. Apply the result as one Yjs update with origin `notion` through the
     same checker as `pages.sync`. Update the bases, and publish
     `pages.page_changed.v1` (`body`).
- **Outbound:** after a member's sync, a debounced job (10 s after the last
  update) diffs the mapped blocks against their bases. It records outbound
  changes for blocks whose hash moved, never for `external` or unresolved
  `conflict` blocks.
- **Conflict operations:** `pages.resolve_conflict { page, bid, keep: mine |
  notion | both }` (`pages:write`), with undo. "Notion's" replaces our block's
  content, "Keep both" turns the conflict into an ordinary block, and either
  way the base moves.
- **Stopping:**
  - `pages.stop_sync`, disconnecting, either side trashing the page, or
    `forget_member` for the connection's owner set `stopped` with a reason.
  - The page stays as it is, and nothing is deleted in Notion.
  - A Notion page the member un-shares in Notion answers 404 on the next run,
    which becomes `stopped`, reason `not_shared`.
- **Moving a synced page to Workspace** is allowed. The Share dialog then
  says: "Edits here go to Notion through Marta's connection." This is
  recorded as a decision under the owner's security-first delegation:
  sharing is the member's choice, and the warning makes the consequence
  visible.

## Access (ADR 0017)

- Connections: the member who made one owns it, no client token may start
  one, and the `Credentials` port only gives tokens to the adapter.
- `pages.import_from_notion` needs a connection the caller owns, and `write`
  on the target parent.
- Inbound runs only touch pages linked to that connection, and links belong
  to the page's workspace. The webhook lookup reads ids across workspaces in
  the maintenance scope and only marks streams due, so nothing crosses a
  workspace.
- Mentions to Notion: a Campbooks record mention becomes the text "Campbooks
  link" with the app's link to it, never its subject or name.

## Events and tracking

- Uses the existing `pages.page_changed.v1` and the connectors change-stream
  and outbound events. No new domain event carries text.
- Tracked events:
  - `notion_connected`;
  - `notion_pages_imported` (count bucket);
  - `notion_sync_applied` (direction, blocks bucket);
  - `notion_conflict_raised` / `notion_conflict_resolved` (choice);
  - `notion_sync_stopped` (reason).
- Coverage rows: the OAuth start and callback, the webhook, the stream run,
  the outbound run, import, resolve, stop, and the two jobs.

## The flag and the owner's setup

- `CAMPBOOKS_CONNECTORS__NOTION__ENABLED` is off by default. When off, there
  is no Connections card, no route, no hook and no jobs, and stray Notion
  settings never stop the server (tested against production values).
- **Owner actions before turning it on in prod:**
  1. Create a **public** Notion integration in Not A Camp's Notion account,
     with read content, update content and insert content, plus user
     information without email.
  2. Set its redirect URI to `https://app.campbooks.not-a-camp.com/auth/callback/notion`.
  3. Put the client id and secret in Infisical (`/campbooks-next`).
  4. Create the webhook subscription in Notion's dashboard for
     `https://app.campbooks.not-a-camp.com/hooks/notion` with the page
     events, and put the verification token in Infisical.

## Risks

- **Notion request limits.** A large first import, such as a 41-page wiki,
  queues within the per-connection bucket. The import screen says how many
  pages and roughly how long.
- **Block-kind drift.** A Notion block kind added later maps to `external`
  until we add it, so nothing is lost.
- **Hash stability.** Content hashes are computed from the normalized
  snapshot (kinds, attributes and rich-text runs in a fixed order), never
  from raw JSON, so Notion re-serializing a block doesn't fake an edit.
