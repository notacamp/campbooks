## Why

Many of our members already keep their notes, roadmaps and wikis in Notion.
Now that Pages is live (notacamp/campbooks#672), the owner asked (2026-10-25)
for "a way to sync with notion", so those pages can live next to the mail
without leaving Notion behind.

The owner chose (2026-10-25):

- **Import, then two-way.** Pages chosen from Notion come into Pages, and
  from then on edits on either side reach the other.
- **Only the pages you pick.** Access is limited to what the member shares
  with Campbooks in Notion's own consent screen.
- **Per member, private.** Each member connects their own Notion account,
  and imported pages start in their Private place.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/376 (epic
#363). Prototype: board "Sync with Notion" on
https://claude.ai/artifact/B4DKyoW5itjeAhcgAaY2rs

Modules touched:

- **`connectors`**: a new provider, `notion`, and a new capability, `pages`.
  It brings its own OAuth flow (Notion's OAuth has no ID token, so it skips
  the identity front door, as `connect_imap` does), a Notion API client, a
  change stream per linked page fed by a signed webhook with polling as the
  fallback, and outbound writes for page changes. ADR 0010 already allows
  this: "adding a provider or a channel is an adapter, not a domain change".
- **`pages`**: consumes the Notion change stream and applies it to the page's
  merged document. It records which block maps to which Notion block,
  handles a conflict as a block the member resolves, and emits outbound
  changes for its own edits. No dependency on `connectors`: they talk through
  change streams and outbound changes, the same way `conversations` and
  `schedule` do.
- **Client**:
  - Settings → Connections → Notion;
  - "Import from Notion";
  - the sync status on a page;
  - the conflict block and its choices;
  - the "Kept in Notion" block.

## What Changes

- **Connect Notion** (Settings → Connections). Notion's consent screen asks
  which pages to share. The connection belongs to the member who made it,
  its tokens are sealed by the instance key and bound to that connection
  row, and the refresh token is rotated on every refresh.
- **Import from Notion** lists the pages the connection can see. The member
  picks some, and each becomes a private page, with its sub-pages under it,
  linked to its Notion page.
- **Two-way sync.**
  - A change in Notion arrives by webhook, about a minute later (or by
    polling where no webhook is set up), and is applied block by block into
    the page's Yjs document, so Campbooks edits to other blocks survive.
  - A change in Campbooks is sent to Notion about 10 seconds after the last
    keystroke, as block updates, appends and deletes, within Notion's
    request limits.
- **Conflicts.** When the same block changed on both sides between two syncs,
  Campbooks keeps its own version in place and adds Notion's version
  beneath it as a conflict block. The member chooses Keep mine, Use
  Notion's or Keep both. Nothing is thrown away until they do, and that
  block isn't sent until then.
- **Blocks Campbooks lacks** (synced blocks, columns, tables, embeds,
  databases, equations…) become a "Kept in Notion" block. Campbooks shows its
  kind and a link, never edits or deletes it, and stores none of its
  content.
- **Mentions** of Campbooks mail, tasks and events go to Notion as a plain
  "Campbooks link", without their subject. A Notion page mention becomes a
  Pages mention when that page is synced too, and a link otherwise. A Notion
  person mention becomes plain text.
- **Stop syncing**, disconnecting, a page trashed on either side, or the
  member leaving the workspace all stop the link. Nothing is deleted on the
  other side.
- **Off by default** behind `CAMPBOOKS_CONNECTORS__NOTION__ENABLED`. Without a
  webhook secret it polls.

## Non-goals

- Notion databases as tables. These wait for Tables (#365).
- Sending new Campbooks files and images to Notion (they go as links in v1).
  Notion's files do come into Campbooks.
- Comments, page properties beyond the title, page icons and covers, and
  workspace-wide connections.
- Live cursors, or seeing who is editing in Notion.
- Exporting a Campbooks page that didn't come from Notion. Every link starts
  as an import in v1.
- Any AI or heuristic reading of page content. The mapping is structural,
  block type to block type.
