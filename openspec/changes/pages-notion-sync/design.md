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
  - Fixed host `api.notion.com`, with `Notion-Version` pinned to
    **2026-03-11**, the version the owner's connection and webhook
    subscription use. Its breaking changes are handled: blocks are added with
    the `position` object (`after_block`, `start` or `end`), not `after`;
    `archived` is gone and `in_trash` is the only trash flag (a page in the
    trash stops the link with `trashed_in_notion`, a block in the trash
    counts as deleted); the `transcription` block is `meeting_notes` and,
    like any kind Pages lacks, is kept in Notion; webhook events carry
    `api_version` and only `2026-03-11` is read (any other is answered 200
    and ignored). Databases are data sources from this version
    (`data_source.*` events, no `database.*`); v1 has no use for them, which
    matters to cloud #377.
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
  and present whenever Notion is on (and the instance secret is set), before
  the webhook secret exists, because Notion posts the subscription's
  verification token to it first.
  - **No webhook secret set**: the only body taken is exactly
    `{"verification_token": <text up to 200 characters>}`. It is kept sealed
    (a purpose of its own under the instance key) as the instance's one row,
    answered 200 and logged as having arrived, never with its value. Every
    other post is 404. The operator reads it with
    `campbooks-server admin notion-webhook-token` (`--clear` forgets it) and
    pastes it into Notion, then sets `CAMPBOOKS_NOTION__WEBHOOK_SECRET[_FILE]`.
    No HTTP endpoint, event or log ever carries it, and the runtime database
    role can replace the row but not read it.
  - Five page events make a linked page's stream read now and nothing else:
    `page.content_updated`, `page.properties_updated` (a page's title is a
    property), `page.moved`, `page.deleted` and `page.undeleted`. After
    `page.deleted` the run finds the page trashed and stops the link with
    `trashed_in_notion`; `page.undeleted` does not re-link (a stopped link
    stays stopped; the member imports again). Any other event type is
    answered 200 so Notion does not retry it, and marks nothing due.
  - **A webhook secret set**: a verification-shaped body is 404 (a token
    cannot be armed again). Every other post is checked in constant time
    against `X-Notion-Signature`, an HMAC-SHA256 of the raw body.
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
     events (with Notion on and no webhook secret set, the hook keeps the
     verification token Notion posts). Read it with
     `campbooks-server admin notion-webhook-token`, paste it into Notion's
     verification screen, put the same value in Infisical as the webhook
     secret, redeploy, and run the command with `--clear`.

## Risks

- **Notion request limits.** A large first import, such as a 41-page wiki,
  queues within the per-connection bucket. The import screen says how many
  pages and roughly how long.
- **Block-kind drift.** A Notion block kind added later maps to `external`
  until we add it, so nothing is lost.
- **Hash stability.** Content hashes are computed from the normalized
  snapshot (kinds, attributes and rich-text runs in a fixed order), never
  from raw JSON, so Notion re-serializing a block doesn't fake an edit.

## Deviations while building

Where the spec turned out to be wrong or short, the closest secure thing was
built. Each is listed here.

- **Webhook before its secret.** The spec had the hook absent until the secret
  was set and answering the verification post with a log line. Notion posts
  the token once and the operator must paste it back, so the hook now exists
  whenever Notion is on, keeps the token sealed while no secret is set, and
  the admin CLI `campbooks-server admin notion-webhook-token` is the one way
  to read it (`--clear` forgets it). With the secret set a verification body
  is 404. See the connectors spec delta.
- **The consent start.** `POST .../connections/notion/authorizations` answers
  this instance's `/auth/start/notion?ticket=...` rather than Notion's URL:
  the ticket is the consent sealed with the instance key, and the state, the
  PKCE verifier and a browser-binding cookie are made when the browser
  arrives, so a ticket opens once and only for the browser that follows it.
- **Snapshot hashes.** The snapshot carries no content hash: the pages module
  computes it from the normalized block, so the connector cannot assert one.
- **Polling.** Without a webhook a stream asks Notion for the page itself
  (one call, its edit time) at least every five minutes, backing off as it
  stays quiet, instead of a search per connection. With a webhook the longest
  poll interval is the safety net.
- **Order and moves.** With the `position` object a new first block under a
  parent can be added first (`start`); a block moved inside a page is not
  followed (it is the same block where it is).
- **Files and images made here** go to Notion as a paragraph that names the
  file and links to the page; their bytes stay in Campbooks.
- **Titles** sync inbound only; a title changed here is not sent.
- **Who edited.** The conflict banner cannot say who changed a block in
  Notion (the API's editor is not read), so it says it changed in Notion.
- **`connectors.notion_connected`** is covered by the existing
  `connection_added` event with provider `notion`, not a new one.
- **No offline access, no connection.** A Notion answer without a refresh
  token is refused (`no_offline_access`).
- **Disconnect** deletes the tokens; Notion has no revocation endpoint to
  call, so access is withdrawn in Notion.
- **Switching Notion off** does nothing destructive: the streams wait and no
  link is stopped; `notion_unavailable` is only used when a link names a
  connection that is not a Notion connection of the workspace.
- **Where the card lives.** The Notion card sits on the existing connections
  page (Settings, mailboxes), whose title the owner may want to rename.
- **Answers to refused asks.** `stop_sync` and `undo_resolve_conflict` answer
  409 (not 404) for "no link" and "no such choice", and `list_notion_pages`
  answers 403 to an admin or client who may see the connection and 404 to
  anyone who may not, to fit the access matrix.
