# Changelog

All notable changes to Campbooks Next are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Security

- **Suspending an account or signing out everywhere stops its access tokens at once.**
  Tokens already issued used to keep working until they expired (up to ten minutes).
  Now the API refuses them on the next request (401 `invalid_token`, the same answer
  as any bad token), on every route, the admin API, the idempotency layer and the web
  app's calls, and an open change feed ends within a heartbeat. Unsuspending does not
  bring them back: the person signs in again. A code or refresh token from a sign-in
  before the revocation gets `invalid_grant`. Each request with a person's token now
  reads one row of `identity.accounts` (migration
  `20261020060000_identity__tokens_not_before` adds one nullable column). Instances
  using an outside identity provider, and workspace clients' tokens, are unchanged.
- **An allow-list for who may sign in (`CAMPBOOKS_SIGNUP__ALLOWED`).** A comma-separated
  list of addresses and domains (a domain matches `*@domain` exactly, not subdomains).
  Unset or empty changes nothing. When set, only listed people can sign in or create an
  account, existing accounts included, and an invitation does not bypass it. It is checked
  at the provider sign-in (the provider-verified address), when a new account is created,
  when a way in is added, at passkey sign-in (the account needs a listed, verified address;
  a passkey cannot create an account) and at every token exchange, so removing someone
  cuts their access at their next refresh. Refusals land on `/sign-in?error=sign_in_not_allowed`
  (or 403 `sign_in_not_allowed`), in all four languages, and say nothing about whether an
  account exists.
- **TLS library updated to rustls 0.23.46**, which fixes RUSTSEC-2026-0346 to 0349: TLS
  secrets in `Debug` output, data sent after an ECH rejection, unbounded certificate
  decompression, and a downgrade to classical key exchange.
- **Only the member who connected a mailbox can disconnect it.** Workspace owners and
  admins still see its health and can sync it now, but get 403 `disconnect_owner_only`.
- **Disconnecting deletes the mailbox's mail by default.** That covers its messages,
  attachments, conversations, labels, drafts, Campbooks' decisions, search entries and
  inbox memberships, and its stored files are removed by the next blob sweep.
  `DELETE …/connections/{id}?mail=keep` keeps the mail instead as read-only history
  (no more syncing or sending). When an owner or admin removes a member,
  `DELETE …/members/{user_id}?mail=keep` keeps the mail of the mailboxes that member
  connected; otherwise it is deleted. A member who leaves loses every inbox
  membership and their drafts, and gets none of it back if they rejoin. The inbox
  list shows a kept mailbox as `read_only`. Keeping is refused (409 `keep_needs_a_reader`) when a
  mailbox would have nobody left to read it. Kept mail can still be read and
  deleted, and its scheduled sends go back to drafts. A disconnect never races a
  sync: a batch landing at that moment is either fully deleted or dropped.
- **A client a member registered acts for that member and no one else.** It reads
  only the inboxes they read, and stops working (401) when they leave the workspace,
  which also removes the clients they registered there. Client tokens no longer get
  any inbox or attention item by themselves. A removed client's open change feed ends within a
  heartbeat. A member removed and invited back can rejoin only once their departure
  has been handled everywhere (409 `still_leaving` until then; a departure that
  failed for good stays among the failed jobs, and blocks, until an operator replays
  it), and a scheduled send
  of someone no longer in the workspace goes back to a draft instead of out.
- **Workspace owners and admins never read an inbox they are not a member of**, and
  the change feed, search and every operation now enforce this through one access
  check. Each resource type declares its owner and who else may do what (ADR 0017).
  `cargo xtask arch` fails any operation that does not declare the access it needs.
- The change feed rechecks access on every page. A member taken out of an inbox gets
  a `reset` (`access_changed`) and no more of its ids. Drafts reach only their author,
  and connections only the member who connected them, owners, admins and client tokens.
- A repeated request with the same `Idempotency-Key` is answered from the stored
  response only while the caller still has the scope, the workspace and the inbox the
  first request used.
- Row-level security now also covers the kernel's change feed, file catalogue, event
  log and job queue. A search cursor only works in the workspace it came from, and
  search reports `indexing` only to callers who can see an inbox.
- **Self-hosters:** three new migrations run on start, with nothing to configure.
- When a way to sign in is added to your account (a Google, Microsoft or Zoho
  account, or a passkey), you are told: by mail to your account's address and
  the new account's address (verified addresses only, in your language, no
  tracking) when the instance has a mail server, and on Settings → Ways to
  sign in, which marks what was added this week, either way. New settings
  `CAMPBOOKS_MAIL__SMTP_URL` (or `_FILE`) and `CAMPBOOKS_MAIL__FROM`; see
  deploy/README.md → Outgoing mail.
- Sign out everywhere (Settings → Ways to sign in): ends every sign-in to
  your account, on every device and in every app.
- A copied web session cookie used after the real one moved on ends the
  session for both, and its tokens at the identity provider.
- Adding a passkey needs a sign-in from the last five minutes (the app signs
  you in again first).
- A sign-in link opened in another browser no longer completes there.
- Refresh tokens end with the sign-in they came from.
- Signing in, authorizing and signing out are rate-limited per address;
  passkey-only sign-ups allow 5 per address an hour (200 per instance).
- Workspace members' addresses are shown only when the identity provider
  verified them.
- The admin API asks for a sign-in from the last hour (was twelve hours), and
  app names that only look like "Campbooks" (full-width, accented, styled
  letters) are refused.
- Upgrading: sign-ins waiting at the moment of the upgrade start again from
  the app; refresh tokens of the native app (90 days) and of other apps (365
  days) now end with the sign-in they came from (30 days), so those apps ask
  to sign in again then; existing members' addresses that the identity
  provider never verified stop showing until their next sign-in verifies
  them; a mail server is only accepted over TLS (`smtps://` or
  `smtp://…?tls=required`), except a relay on the same machine.
- **Mail no longer goes to an AI provider outside your network unless you say
  so.** Each AI role's address is checked: a loopback or private address,
  `localhost`, a single-label name such as a compose service (`laya`,
  `ollama`) or a host you list in `CAMPBOOKS_AI__INTERNAL_HOSTS` counts as
  your network; anything else gets no mail content until you set
  `CAMPBOOKS_AI__<ROLE>__EXTERNAL_CONTENT=allow` for that role. Without it the
  gateway refuses (`ai_external_content_refused`) and the decide pass leaves
  sorting to the rules. Admin → System shows where each role is and whether it
  receives mail. **Self-hosters using a hosted provider:** add the opt-in for
  each such role, or it stops being used after this upgrade.
- Campbooks no longer follows HTTP redirects from AI providers, Gmail, Zoho
  or the identity provider's key endpoint: a redirect could have carried mail
  or tokens to another host. A 3xx is reported as an error.
- Errors no longer carry mail: an AI provider's error body is not kept, and
  logs and failed jobs describe database and JSON errors by kind and position
  (SQLSTATE, constraint, table), never the values they quote.
- API responses (`/api/`, `/bff/`, `/admin/`) carry `Cache-Control: no-store`
  and `Pragma: no-cache`, so browsers and proxies don't keep them.
- `pnpm record` refuses any base URL other than `localhost`, `127.0.0.1` or
  `[::1]` unless `--allow-remote` is passed.
- A failed job's error is cleaned before it is stored and logged: only its
  first line, at most 500 characters, with quoted text, email addresses,
  link paths and queries, and long tokens removed. Admin → Jobs shows the
  cleaned message. Errors already stored are left as they are.

### Removed
- **Due dates on needs, until they are extracted from the mail.** A need no longer
  carries a due date guessed from words like "Friday" or "end of the month"; it will
  again once a model extracts the date (a follow-up of ADR 0018). Needs raised before
  the upgrade keep theirs.
- **Rauthy is gone from the shipped stack** (ADR 0014, last slice). `deploy/compose.yaml`
  no longer has the `idp`, `idp-init` and `idp-provision` services or the `idp-data`
  volume, `deploy/idp/` is deleted, and the stack no longer sets
  `CAMPBOOKS_AUTH__ISSUER`, so a default install signs in through Campbooks' own
  issuer. **To upgrade:** copy the new `compose.yaml`, then
  `docker compose up -d --remove-orphans` removes the three old containers (leaving
  them running breaks nothing, they are just unused; an old compose file kept as it is
  keeps working on Rauthy). The `idp-data` volume is left for you to delete once you are
  sure (`docker volume rm <project>_idp-data`). **Accounts that lived in Rauthy are not
  carried over:** those people sign in again with Google, Microsoft, Zoho or a passkey
  and get a new Campbooks account. Move the `CAMPBOOKS_IDP_*` sign-in provider settings
  to `CAMPBOOKS_GOOGLE__*`, `CAMPBOOKS_MICROSOFT__*` and `CAMPBOOKS_ZOHO__*`, put each
  client secret in `deploy/secrets/<provider>-client-secret` (now mounted into the
  server at `/run/secrets/campbooks`), and add
  `${CAMPBOOKS_PUBLIC_URL}/auth/callback/<provider>` to each provider's OAuth app (see
  `deploy/README.md` -> Built-in sign-in and Upgrading from the Rauthy sign-in).
  `CAMPBOOKS_IDP_*`, `CAMPBOOKS_BFF__IDP_INTERNAL_URL` and
  `CAMPBOOKS_BACKCHANNEL_LOGOUT_URL` no longer do anything in the shipped stack.
  Pointing `CAMPBOOKS_AUTH__ISSUER` at an outside OpenID Connect issuer is still
  possible, but nothing is provisioned for it.

### Changed
- **A decision model sorts mail and raises needs; rules only read headers and can
  only say no** (ADR 0018). Every phrase list and regular expression that read an
  email's subject, body or attachment names is gone. What is left reads structured
  facts only: list mail (`List-Id`, `List-Unsubscribe`, a bulk `Precedence`), mail
  sent automatically (`Auto-Submitted`) and no-reply senders need no reply, and that
  "no" wins over the model's "yes". The `decide` model's answers now act where the
  eval shows they hold their target: it places a new conversation in a stream
  (stream source `model`, never over a member's move) and raises its need (`pay` is
  an action asked by mail it sorted to Receipts). Without a trusted `decide` model,
  mail stays in Primary and no need is raised beyond what the headers settle. The
  upgrade keeps the old rules' answers as a record but stops them counting,
  withdraws the needs only they raised, and asks a trusted model again about each
  recent conversation's newest message. `CAMPBOOKS_AI__DECIDE__PATH` sets where
  decide requests go (default `systemone` for Laya, `chat/completions` for an
  instruct model), so a hosted decision model such as Jev through OpenRouter can
  serve `decide` (see `deploy/README.md`; mail leaves the instance only with
  `CAMPBOOKS_AI__DECIDE__EXTERNAL_CONTENT=allow`). The shipped defaults stay
  open-weight.
- Admin → People marks people whose account is suspended, and Admin →
  Signups → Codes says what a code is: good for the number of signups set
  when it is minted, until it is used up, expires or is revoked.
- The event log is no longer kept forever: events older than
  `CAMPBOOKS_EVENTS__RETENTION_DAYS` (default 90) are deleted hourly, except
  events still being delivered, and failed jobs older than
  `CAMPBOOKS_JOBS__FAILED_RETENTION_DAYS` (default 30) go too. Deleting a
  workspace now also deletes its events and finished jobs when it is purged.
  The first prune after upgrading may take a while on a large instance.

### Fixed
- **Notifications with a question mark no longer ask for a reply.** "Your order has
  shipped. Questions? Contact our support team." raised a Reply need, because a rule
  read any "?" as a question for you. Needs raised that way are withdrawn on upgrade.
  Automated mail (a calendar invitation, a support notice) becomes something to do
  only when the model also finds a deadline in it, and a newsletter never does.
- **A connected mailbox no longer loses its way to sign in to the cleanup.**
  The background cleanup that removes a way to sign in left behind by a
  mailbox connect that never finished now checks that the mailbox really
  isn't connected first, and waits for a connect you retried. If connecting
  a mailbox breaks unexpectedly, you now land back on Mailboxes with an
  error instead of a server error page.
- **The inbox fits the window on desktop.** Between 1024px and about 1180px wide the
  open conversation no longer drops below the list; it stays beside it. A long
  subject, sender or address with no spaces now wraps inside the reader instead of
  making the page wider than the screen.
- **What needs you includes mail from before the upgrade.** Home's "What
  needs you" only showed mail that arrived after attention shipped. After
  this upgrade, each inbox's conversations from the last
  `CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS` days (default 14) get their
  needs within a few minutes, in the background, from the answers already
  stored: no AI call, nothing to run. Replied, archived, spam and trashed
  mail is handled as for new mail, and mail older than the window stays off
  Home. Every open need is looked at again too, so Check questions that rest
  only on unchecked model answers are withdrawn on upgrade.
- **The decisions model's answers are only used once it gets obvious emails
  right** (campbooks-cloud#128). The default Laya model said "yes" to almost
  every question (needs a reply, has a deadline, looks like phishing), often
  at 100% confidence. These answers never acted on their own, but they showed
  up as suggestions and as Check questions on Home. It wasn't a bug in
  Campbooks: the model can't answer these questions without training on them.
  Every server now asks the `decide` role about eight made-up emails with
  obvious answers at startup, again every 6 hours, and every 15 minutes while
  it fails. A model that gets fewer than 90% right isn't used: the rules sort
  mail exactly as if no model were set up, and Admin → System says so
  ("Answers fail the check", with the count). Laya currently fails it, so on
  a default install the rules alone sort mail until a better decisions model
  ships.
- Model answers stored before this release are marked as unchecked (database
  migration). They are kept, but no longer feed suggestions or Check
  questions; Check questions they raised are withdrawn the next time their
  conversation is refreshed.
- The decisions model is now told what Campbooks is, whose inbox the email
  arrived in (personal or shared), what the rules noticed, and that the email
  is data, not instructions. Its yes/no questions are asked as questions.
- **Home no longer asks about mail the rules already settled.** A model
  suggestion is put to you as a question ("Did GitHub ask you to do
  something?") only when no answer Campbooks acts on already said no; a
  plain notification stays off Home. Questions raised that way before this
  fix are withdrawn the next time their conversation is refreshed.
- **Security alerts about your code are a to-do.** A security alert or
  vulnerability notice (in all four languages) from a code host or package
  registry (GitHub, GitLab, Bitbucket, npm, PyPI, RubyGems, crates.io, Snyk)
  now shows on Home as Do and sorts to Updates, but only when your mail
  provider authenticated the sender (DMARC, or DKIM for the sender's own
  domain), the sender's name matches its domain, and nothing in it looks
  like phishing or marketing. Other services' alerts are at most a question,
  and other notifications from the same service ask nothing. Gmail mailboxes
  now keep Gmail's own `Authentication-Results` with each message for this
  (Zoho mailboxes keep none until Zoho's header has been checked, so no
  alert is a to-do there yet). Mail in spam or the trash never shows on
  Home.
- `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER` (default `https://accounts.zoho.com`): the Zoho
  accounts server sign-in sends people to. A Zoho app registered in one data centre
  only (for example the EU console) is rejected as an invalid client by
  `accounts.zoho.com`; set this to your app's data centre (`https://accounts.zoho.eu`).
  It must be one of Zoho's accounts servers, or Campbooks refuses to start.

### Added

- **A daily AI limit per workspace (`CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE`, default
  5000, `0` for none).** Every model call (`decide`, `embed`, `complete`, `vision`) is
  now counted per workspace, role and feature, per UTC day, with the tokens the
  provider reports, in two new tables of the `system` schema (migration
  `20261020180000_system__ai_usage`). Past the limit a workspace's new mail is still
  stored and sorted by the facts and taught rules, without the model pass, until
  00:00 UTC; nothing errors, and skipped mail is not asked again later. One log line
  says when a workspace reaches it. If the count cannot be written, the call is not
  made and the job retries. Administrators read a day's usage per workspace, capped
  ones first, at `GET /admin/v1/system/ai-usage` (`system.read`). Self-hosters
  running their own model can set `0`.
- **Teach once in the inbox.** After you move, archive, mark read or clear one
  email, Campbooks offers to do the same to every one like it, and to new
  ones: it says which mail and how many, shows a few, and makes a rule with
  one key (1). The rule's run shows its progress with Undo all; rows it
  handled say so ("Campbooks moved this · like 23 others from …"), with Stop
  doing this in the reader; changing back the same rule's work twice in a
  month asks whether to keep it. The rules list for Scout's memory is ready
  for its page. Demo: `pnpm --dir clients fake-google --lookalikes 24` and
  `pnpm --dir clients record teach-once`.
- **Rules you teach, applied to the mail that looks the same.** A taught rule
  says which mail it is about (the sender, its domain, a mailing list, the
  subject's template or words, a header, a label or a stream) and what to do
  (move it to a stream, archive it, label it, mark it read, say whether it
  needs a reply or an action, or always or never show it). A rule's answer
  beats the built-in signals' and any model's, always gives the same result
  for the same mail, and never hides mail that looks suspicious or waits for
  your review. Creating one applies
  it to the mail already in the inbox in the background, a hundred
  conversations at a time, with progress; removing it can put that mail back.
  Moving one back by hand makes it an exception the rule leaves alone. New
  operations: `conversations.preview_rule`, `create_rule`, `confirm_rule`,
  `update_rule`, `remove_rule`, `list_rules`, `get_rule`, `suggest_rules`
  and `dismiss_rule_suggestion`; new event `conversations.rule_changed`. The
  screens that use them follow in the next release.
- `conversations.need_changed` may clear a need with cause `rule`.
- Settings → Mailboxes: every mailbox connected to the workspace (yours;
  all of them for owners and admins), who connected it and whether it is
  syncing, waiting for someone to sign in again, paused by the provider or
  failing. Connect Gmail or Zoho Mail from there (only the providers your
  instance has set up for mail), choosing whether you also sign in with that
  account (untick it for a shared mailbox like support@, which then never
  becomes a way in); reconnect a mailbox of yours that stopped; or
  disconnect one, choosing whether its mail in Campbooks is deleted (the
  default) or kept as read-only history. The mail at Gmail or Zoho is never
  touched; a way to sign in that came only from connecting that mailbox
  goes with it. Keys: ⇧C connect, J/K choose, R reconnect, ⌫ disconnect. Home's
  "Connect your mail" now opens it, and Settings lists its sections
  (Members, Mailboxes, Ways to sign in) in the sidebar and, on phones,
  under the header. API: `identity.list_ways_in` returns
  `mailbox_providers`; `identity.start_link` takes `sign_in` for a connect
  (default `true`) and adds no way in when it is `false`; a way in that came
  only from connecting a mailbox is removed when that mailbox is disconnected
  (unless another connection of yours uses it or it is your last way in),
  signing out whoever used it, and a connect that fails (mail access declined,
  or the mailbox refused) leaves no new way in behind;
  `connectors.connection_added` now names the provider account and the
  grantor's account and is published for every accepted grant; connections
  show `granted_by_you` and members
  `you` (the app no longer works out who you are by matching email
  addresses). An inbox kept as history after a disconnect opens read-only,
  saying so, with nothing to act on; new messages never offer it. Two
  migrations (link tickets, and the connections a way in brought); nothing
  to do.

- **Move through the inbox with the arrow keys.** ↓ and ↑ move through a
  stream's conversations like J and K, Enter opens one, and going past the
  last row loads more. In an open conversation the arrows scroll it (J and K
  still go to the next or previous one), and they're never taken from a
  field, a menu or a dialog. The `?` sheet and the empty reader show "J or ↓".
  Home's "what needs you" list moves with the arrows too (an open item scrolls).
- Search in the app. Press `/` anywhere in a workspace for a search panel:
  instant rows by relevance with the words that matched marked, the filters
  Campbooks understood (`from:`, `has:attachment`, `is:unread`, dates...)
  as chips you can remove, ↑ ↓ to move, ↵ to open the conversation in the
  inbox reader, Ctrl/⌘↵ for all results and Esc to close. ⌘K now ends with
  "Search conversations for ..." when you have typed something. The results
  page (`/w/<workspace>/search?q=`) lists the hits newest first, with a
  "Most relevant" toggle, quick filters for attachments and unread mail, and
  the reader beside the list on wide screens; J, K, ↵, E, S, U, M and Z work
  as in the inbox. On phones the top bar has a search button, the page is
  full screen and the reader is its own screen. Recent searches are kept on
  this device only, for the signed-in account and workspace (cleared with one button and on sign-out); older mail still being
  indexed and every error are said plainly. In English, European Portuguese,
  Spanish and French.
- Admin → Jobs: administrators with the `jobs.read` permission see the
  background queue by kind (waiting, scheduled, running, running with a lost
  worker, failed) and the jobs that gave up with their last error (monospace,
  wrapped, copyable). With `jobs.retry` they can retry a failed job, retry all
  failed jobs of a kind, or discard one after a confirmation. Only failed jobs
  are ever touched, repeating an action changes nothing, and a job's contents
  are never shown or returned. Grant the permissions with
  `campbooks-server admin grant --permission jobs.read --permission jobs.retry`
  (or `super`).
- Admin → People and Admin → Workspaces, for administrators (ADR 0015):
  everyone who has signed in and every workspace, searchable and paged,
  with a person's workspaces and roles, how they sign in (ways in, passkeys,
  live sessions, last sign-in), and a workspace's members, pending
  invitations (counted) and each connected mailbox's health and sync state.
  Metadata only: no mail, subject or document is ever shown or counted.
  Needs the `users.read` and `workspaces.read` admin permissions (`super`
  has both).
- Suspending an account (admin permission `users.suspend`): the person is
  signed out everywhere at once (issuer sessions ended, refresh tokens
  revoked, web sessions dropped) and cannot sign in again until unsuspended;
  nothing else changes. Every suspension is recorded in the account's audit
  trail. An administrator's account cannot be suspended until its admin
  profile is revoked with `campbooks-server admin revoke`. Access tokens a
  native app already holds stay valid until they expire (at most ten
  minutes).
- Admin → Signups: administrators with `signups.review` (or `super`) see who asked to
  join an instance that approves new accounts and approve or reject each request
  (a rejection can carry a note only administrators see), and mint, copy once and
  revoke **signup codes** (each good for the number of signups you set) for
  instances that ask for a code. A person waiting for approval, approved, or not
  approved sees that on the create-workspace screen the next time they open the
  app (they are not emailed yet). Codes are
  stored as digests, work for the number of uses and until the day you set, and are
  spent when the workspace is created, so two people cannot share the last use.
  `CAMPBOOKS_SIGNUP__CODES` keeps working as before (reusable codes), and
  `CAMPBOOKS_SIGNUP__POLICY=code` no longer needs it to start. A refused request
  now says `signup_rejected`, and asking again no longer repeats the request.
- What needs you. Home now lists what your mail asks of you, grouped Review,
  Reply, Decide, Do and Pay, due soonest first: a question you owe an answer
  to, a request, a bill, a phishing warning. Each says why in a sentence and
  quotes the sender's own words; the numbers stay behind "How Campbooks
  decided". J/K, Enter, R reply, E done, L later, X not needed (or "It's
  safe"), Z undo, and "Done today" lists what left your list. The inbox shows
  the same as chips on rows and in the reader's note, which no longer shows a
  percentage. It runs on the rules alone (no AI role needed); replying from
  any client, archiving a reply or a warning, and the trash clear an item;
  bills and requests stay until Done. Only mail from the last
  `CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS` days (default 14) counts. A new
  module, `attention`, with the scopes `attention:read` and `attention:write`
  (the web app gets them at the next sign-in). "Done today" and what is
  overdue follow your device's time zone; deadline words ("Thursday") are
  read against the message's date in UTC until accounts carry a time zone. Migrations add
  `conversations.needs` and the `attention` schema; nothing to do.
- Apps other than Campbooks on the web can use your account, with your say
  each time. A consent screen names the app, says whether anyone vouches for
  it, lists what it will be able to do in plain words, and asks which
  workspace it may use; an app on your device (the desktop app, a local
  agent) is asked for every time and comes with a warning. Allow starts
  unfocused, has no shortcut and waits a second after the page shows, so no
  stray key press answers it; a remembered answer lasts 90 days for the same
  return address. Settings → Apps and integrations lists what you allowed,
  with Revoke, which ends the app's tokens at once. Allowing needs a sign-in
  from the last ten minutes.
- The desktop and mobile apps' sign-in is on (`campbooks-native`), always
  through the consent screen.
- A workspace's own apps: owners and admins register server-to-server apps
  (`client_credentials`, a secret shown once, rotate, remove) and sign-in
  apps under Settings → Apps and integrations. Their tokens work in that
  workspace only.
- MCP agents can connect: dynamic client registration (`/oauth2/register`,
  off unless `CAMPBOOKS_OAUTH__DYNAMIC_REGISTRATION=on`,
  rate-limited, public clients only, deleted after 30 days unused), client
  metadata documents (off unless `CAMPBOOKS_OAUTH__CLIENT_METADATA_DOCUMENTS=on`;
  an `https` client id, fetched with SSRF protection and cached up to a day), protected-resource metadata
  (`/.well-known/oauth-protected-resource/api`), `resource_metadata` in the
  API's `WWW-Authenticate`, and resource indicators. Apps that aren't
  Campbooks' own can never hold an `identity:*`, `workspaces:write` or
  `workspaces:admin` scope, get read scopes only unless they ask for more,
  see only the workspace they were allowed in, and can't be called
  Campbooks or Not A Camp.
- Search over your mail (`GET /api/v1/w/{workspace_id}/search`, operation
  `search.query`, new scope `search:read`; screens follow). Words match in
  English, Portuguese, Spanish and French whatever language you type in, with
  accents ignored ("fatura joao" finds "As faturas de João"); Gmail-style
  operators (`from:`, `to:`, `cc:`, `subject:`, `has:attachment`, `in:`,
  `is:unread`, `is:starred`, `label:`, `before:`, `after:`, `older_than:`,
  `newer_than:`, `"phrases"`, `-word`, `OR`) narrow it, and highlights come
  back as plain text. You only ever find conversations in inboxes you can
  read, checked at the moment you search. The index lives in Postgres and
  stays current within seconds; mail that arrived before this version is
  indexed in the background the first time someone searches a workspace (or
  run `campbooks-server search reindex --workspace <id>`). Queries are never
  stored or logged. **Self-hosters on managed Postgres:** the server now
  creates the `unaccent` and `pg_trgm` extensions (both ship with Postgres and
  are in the default image); allow them if your provider asks.
- The Visor (the assistant's face) has three new emotions: dizzy (something failed on
  our side), starry (a big win) and heart eyes (a warm moment), with an optional
  gold/rose tint. Its thinking motion is now a typing-indicator hop that reads at
  small sizes.
- Passkeys. Add one under Settings → Ways to sign in (Add a passkey, `N` `P`), then sign
  in with it on the sign-in screen: the passkey button (`P`) or your browser's
  autofill. You can also create an account with a passkey alone, no Google, Microsoft or
  Zoho needed (not on an invite-only instance, where an invitation is matched by email
  address). Passkeys are ways in like provider accounts: rename or remove them (never your
  last way in; removing one signs out whoever used it), and adding or removing one needs
  a sign-in in the last ten minutes. They need `https://` (or `http://localhost`), and are
  bound to the host of `CAMPBOOKS_PUBLIC_URL`: changing that host retires them.
- A ready-made image for the local `decide` model: `ghcr.io/notacamp/campbooks-laya`,
  published for linux/amd64 and linux/arm64 with the same tags as the Campbooks image
  and scanned the same way. `docker compose --profile ai` now pulls it instead of
  building `deploy/laya` on your machine (the `build:` line is still there, commented,
  if you prefer to build it). The model checkpoint still downloads into the
  `laya-models` volume on first start. Pin it to the same tag as the server image.
- Zoho Mail. Signing up with Zoho also connects your Zoho mailbox (Zoho
  asks a second time, for the mail, only when the account is new; signing
  in again never asks), and members can connect more Zoho logins; every
  mail account of a login becomes its own inbox, beside Gmail in the same
  streams. Mail is read from the account's own data centre and polled (Zoho
  has no push); read, star (flag), archive and unarchive go back to Zoho;
  unarchiving never moves your own sent mail into the Inbox; replies and new
  messages are sent through Zoho, never twice. Uses the Zoho
  app of Sign in with Zoho, which must allow `ZohoMail.accounts.READ`,
  `ZohoMail.folders.READ` and `ZohoMail.messages.ALL`. New settings
  `CAMPBOOKS_CONNECTORS__ZOHO__*` (window, size limits,
  `REQUESTS_PER_MINUTE` per Zoho login, default 60, and `SWEEP_MINUTES`).
  Zoho labels are not synced yet. A migration renames the connectors' message
  index (`connectors.messages`); nothing to do.
- Attachments open from the reader. Each message lists its files with their
  size, Download and, for pictures and PDFs, Preview: a picture opens in a
  dialog, a PDF in your browser's own viewer in a new tab. V moves through
  the attachments, Enter opens one. HTML, SVG and every other type only
  download. The new operation `conversations.get_attachment` (scope
  `conversations:read`) serves the bytes as a download under a cleaned file
  name, with the stored type only when it is on a safe list (pictures and
  PDFs must also start like one) and a `sandbox` policy. Pictures inside a message's body (`cid:`) now show without asking,
  since nothing remote loads; remote images that stay hidden show a quiet
  placeholder instead of a broken-image icon. Mail synced before this
  release keeps the placeholder for its inline pictures. The web client's
  Content-Security-Policy now allows `blob:` images (the picture preview).
- Reply, reply all, forward and new messages, sent through your Gmail. The
  composer opens under the thread (R, A, F) or on its own screen (C): From
  your mailbox, To, Cc and Bcc as chips, the quoted history folded under your
  text, forwarded attachments you can drop, and plain text with basic
  formatting (⌘B, ⌘I, lists). Drafts save as you type and come back when you
  reopen the conversation. ⌘↵ sends after a 10-second window in which Undo
  takes it back (`CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS`, 0 to 60). A
  send never goes out twice, even after a timeout or a restart, and the sent
  message appears in its conversation at once. Sending needs the `sender`
  or `manager` role on the inbox and the new `conversations:send` scope,
  which the web app asks for by itself; mailboxes already connected need no
  new consent.
- What you do to mail in Campbooks now happens in Gmail too: marking read or
  unread, starring, archiving and moving back to the inbox, adding or
  removing a Gmail label, and moving a conversation to Primary, Updates or
  Newsletters (Gmail's Primary, Updates and Promotions tabs). Changes reach
  Gmail on the mailbox's next sync, normally within a second, and never come
  back as new changes. If the same thing was changed in Gmail after you
  changed it in Campbooks, the later change wins and Campbooks shows it.
  Throttling and outages are retried with backoff within the shared Google
  quota; a change Gmail refuses is recorded (without any message content)
  and Campbooks shows the message as Gmail has it. A change that could not
  reach Gmail within 7 days (a long pause, a reconnect) is dropped rather
  than made late, mail in Gmail's trash or spam is never brought back to the
  inbox, and only read, star, inbox, category and your own labels are ever
  touched. Labels you create in
  Campbooks and the Receipts stream stay in Campbooks for now. New table
  `connectors.outbound_changes` (migration applied on start).
- The inbox: the four streams (Primary, Updates, Receipts, Newsletters) under
  Inbox with their unread counts, each stream's conversations newest first,
  and a reader with the thread (older messages folded, attachments listed).
  Campbooks' sorting shows only as a suggestion ("Suggests Newsletters"),
  with "Why?" listing the stored reasons and "Move to …" one key away; Done,
  star, read/unread and move all have shortcuts and Undo. On phones the list
  and the reader are separate screens. New read operation
  `conversations.list_streams`; conversation summaries carry `suggestion`
  and details carry `decisions`.
- Email bodies are sanitised on the server before they reach the app
  (scripts, style sheets, forms, frames and tracking pixels removed, links
  open in a new tab), shown only in a sandboxed frame, and remote images stay
  blocked until you choose "Show images" for a message.
- Instance administrators (ADR 0015). An operator makes someone an
  administrator from the server's command line,
  `campbooks-server admin grant --email you@example.com --permission super`
  (`docker compose exec server …`; also `revoke`, `list` and `log`), never
  from the app or the API. Administrators get an **Admin** section in the
  app (account menu → Admin) with a System panel: version, uptime, database
  and migrations, the effective configuration without any secret, and
  whether each AI role's provider answers. The admin API is separate
  (`/admin/v1/*`, `openapi/admin.json`), answers 404 to everyone else, asks
  for a sign-in less than 12 hours old, and records every request in an
  audit trail. A new `admin` database schema is created by the migrations.
  See `deploy/README.md` → Administrators.
- The sign-in screens are Campbooks screens, in English, Portuguese, Spanish
  and French. Opening the app signed out goes straight to "Sign in to
  Campbooks" (and back to the page you opened afterwards), with one button
  per way in this instance offers (keys `1`, `2`, `3`; `P` for a passkey once
  passkeys arrive). Before you leave for Google it says what the one consent
  covers (your Gmail is connected too, what Campbooks keeps, that you can
  untick mail access) and that Google may warn the app isn't verified yet.
  A first sign-in shows the new-account screen (your name and address, and
  what happens next); every sign-in problem is explained with what to do.
- Right after you create your first workspace, "Your Gmail … will be
  connected to this workspace": Connect, or Not now. Home keeps offering it
  (Connect, or forget it) until it expires after 7 days, and names the
  mailbox once it is connected.
- Sign in with Microsoft (personal, work and school accounts;
  `CAMPBOOKS_MICROSOFT__CLIENT_ID`, `__CLIENT_SECRET_FILE`, `__TENANT`, default
  `common`) and with Zoho in every data centre (`CAMPBOOKS_ZOHO__CLIENT_ID`,
  `__CLIENT_SECRET_FILE`), when Campbooks is its own identity provider. Both take
  `__REDIRECT_PATH` like Google. Register `/auth/callback/microsoft` and
  `/auth/callback/zoho` (see `deploy/README.md` → Built-in sign-in).
- More ways in: Settings → Ways to sign in lists how you sign in, adds another
  Google, Microsoft or Zoho account, and removes one (never the last). Every
  mailbox you connect becomes a way in too. A provider account that is already
  another Campbooks account's way in is refused, never merged. Adding or removing
  a way in, or connecting a mailbox, needs a sign-in in the last ten minutes.
  Removing a way in signs out every session that signed in with it.
- `CAMPBOOKS_TRUSTED_PROXIES` (comma-separated addresses or CIDR blocks): the
  reverse proxies whose `X-Forwarded-For` names the client. Set it behind a proxy,
  or the per-address sign-in rate limits count every client as one (see
  `deploy/README.md` → Built-in sign-in).
- `CAMPBOOKS_GOOGLE__REDIRECT_PATH` (default `/auth/callback/google`): the callback
  path Campbooks sends to Google when your OAuth app registered another one, with a
  reverse proxy rewriting it to `/auth/callback/google`. The same
  `CAMPBOOKS_<PROVIDER>__REDIRECT_PATH` pattern applies to future sign-in providers.
- Campbooks can be its own identity provider (ADR 0014), replacing Rauthy in
  a later release. With `CAMPBOOKS_AUTH__ISSUER` unset and
  `CAMPBOOKS_SECRET_KEY` set, the server is an OAuth 2.1 / OpenID Connect
  issuer on its own origin (`/.well-known/openid-configuration`,
  `/oauth2/*`): Ed25519 signing keys generated, encrypted at rest and
  rotated automatically, authorization code with PKCE only, refresh tokens
  that rotate and revoke the whole sign-in when one is reused, and the web
  app signing in through it with nothing to provision. People sign in with
  Google; a new person's mailbox comes with the same consent and is
  connected when they create their first workspace. Accounts are linked to
  a Google account by its id, never by email address. Add
  `${CAMPBOOKS_PUBLIC_URL}/auth/callback/google` to your Google OAuth client
  (see `deploy/README.md` → Built-in sign-in). The shipped `compose.yaml`
  still uses Rauthy, so existing installs see no change yet; the sign-in
  screens arrive in the next release. The built-in issuer needs an https
  `CAMPBOOKS_PUBLIC_URL` (http only with `CAMPBOOKS_ENV=development`), refuses
  to start if `CAMPBOOKS_SECRET_KEY` cannot open its stored signing keys,
  and on an `invite_only` instance creates accounts only for invited
  addresses.
- The new app's first run: a calm signed-out page with one "Sign in" (that
  brings you back to the page you opened), then creating your first
  workspace one question at a time (its name, then Business or Personal,
  which picks its colour), with Skip where a default works. Instances that
  require a signup code, an invitation or an administrator's approval each
  say so plainly.
- Workspaces in the address (`/w/<id>/…`), a sidebar (a header and tab bar
  on phones), the workspace switcher (⌘⇧W, Ctrl+Shift+W elsewhere) with each
  workspace in its own colour, and an account menu with theme, density,
  language and Sign out. The app reopens the workspace you used last on that
  device.
- An empty Home ("Nothing needs you yet") with the shortcuts that work today;
  "Connect your mail" says it is coming, because mail connections are not
  built yet.
- Settings → Members: owners and admins change roles (with Undo), remove
  members after confirming in place, invite people by email and see pending
  invitations. Campbooks does not email invitations yet, so right after
  inviting you get the invitation link once, with a Copy button (C), to send
  it on yourself; inviting again gives a fresh link. The link opens a page
  that signs you in if needed and joins the workspace.
- Conversations, the first piece of the inbox: inboxes, conversations and
  messages (email first), threaded by the provider's thread or by replies,
  with bodies and attachments kept in blob storage. Conversations are sorted
  into Primary, Updates, Receipts and Newsletters, and can be read, marked
  read or unread, starred, archived, labelled and moved to another stream
  through the API; a move is remembered as a correction. Connecting a Gmail
  account creates its inbox (the member who connected it manages it) and
  fills it with the synced mail; other members see an inbox once they are
  added to it, and only those who can send from it can change its
  conversations. Conversation changes reach the live updates stream, shown
  only to the inbox's members.
- Every new message gets the AI pass when the `decide` role is configured;
  its answers are recorded as suggestions only and change nothing yet. Mail
  older than `CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` (default 14) gets
  no AI pass, so importing an old mailbox costs no AI.
- Connect a Gmail account (API only for now; the screen comes next): Campbooks
  asks Google for consent, keeps the grant encrypted with the instance secret
  key, reads the last 90 days of mail and then follows new mail and label
  changes, every minute while the mailbox is busy and backing off to every few
  hours when it is quiet. Each connection shows its health (ok, needs
  reauthorisation, throttled, failing); disconnecting, leaving the workspace or
  deleting it revokes the grant at Google. Off until you register a Google
  OAuth client (`CAMPBOOKS_GOOGLE__CLIENT_ID`,
  `CAMPBOOKS_GOOGLE__CLIENT_SECRET_FILE`; see `deploy/README.md` → Connected
  accounts); tuning is `CAMPBOOKS_CONNECTORS__*`, including the largest
  message kept (`…__GMAIL__MAX_MESSAGE_BYTES`, 50 MiB) and the bytes one sync
  run may download (`…__GMAIL__RUN_BUDGET_BYTES`, 64 MiB). The web app's
  callback screen at `/connections/google/callback` is not built yet, so
  connecting goes through the API for now. Note: replacing the instance
  secret key now also asks members to connect their accounts again.
- Live updates: the app learns what changed in a workspace (renamed, a
  member joined, left or changed role) as it happens and refreshes only what
  it shows, catching up after a lost connection or a sleeping laptop. API
  clients can follow the same stream at `GET /api/v1/w/{workspace_id}/changes`
  (Server-Sent Events, ids only). Self-hosters behind a reverse proxy must
  let `text/event-stream` responses through unbuffered; tuning lives in the
  new `CAMPBOOKS_CHANGES__*` settings (see `deploy/.env.example`).
- Sign in with Google, Microsoft or Zoho. Set a provider's client ID in
  `.env` (`CAMPBOOKS_IDP_GOOGLE_CLIENT_ID`, `CAMPBOOKS_IDP_MICROSOFT_CLIENT_ID`,
  `CAMPBOOKS_IDP_ZOHO_CLIENT_ID`) and put its secret in
  `deploy/secrets/<provider>-client-secret`; `idp-provision` registers it in
  the identity provider and the login page shows its button. Whether a
  provider may create accounts for new people follows
  `CAMPBOOKS_SIGNUP__POLICY` (not under `invite_only`), overridable with
  `CAMPBOOKS_IDP_PROVIDER_SIGNUP`. Microsoft's tenant
  (`CAMPBOOKS_IDP_MICROSOFT_TENANT`, default `common`) and Zoho's data centre
  (`CAMPBOOKS_IDP_ZOHO_ISSUER`, default `https://accounts.zoho.eu`) are
  settings. Existing installs grant the IdP's `campbooks` API key the
  "Auth Providers" right once in the admin UI before enabling a provider. See
  `deploy/README.md` → "Sign in with Google, Microsoft or Zoho".
- A way to measure the AI's sorting before it acts: a fictional, labelled set
  of 320 emails in English, Portuguese, Spanish and French
  (`crates/ai/evals/decide/mail-triage/`, with its labelling guide), and
  `cargo xtask eval decide`, which reports for each triage question (stream,
  needs a reply, asks you to do something, deadline, urgency, suspicious) how
  often each strategy is right, how much mail it could handle on its own at
  the confidence needed, its calibration and its speed. Model strategies run
  against any endpoint set in `CAMPBOOKS_EVAL_LAYA_URL`,
  `CAMPBOOKS_EVAL_INSTRUCT_URL` or `CAMPBOOKS_EVAL_EMBED_URL` and are reported
  as not run otherwise.
- Deterministic email signals, learning from your own corrections (nearest
  neighbours) and cascades of them behind one interface with the `decide`
  model, each answer carrying the reasons behind it.
- The server now answers in your language: error messages from the API are
  in English, European Portuguese, Spanish or French, following the language
  you chose (once accounts can save one) or your browser's language, and say
  which one they use in `Content-Language`. Translations are checked in CI
  (`cargo xtask i18n`).
- Design system for the new app: light, dark and system themes; three
  workspace accents (business in ink, personal in terracotta, forest) and a
  compact density, remembered on each device and applied before the page
  first paints; Funnel Sans and Funnel Display, self-hosted; and the shared
  components the new screens are built from (buttons, keyboard hints,
  avatars, signals, the assistant's visor and note, toasts with undo, empty
  and loading states, queue rows, conversation rows, navigation, tab bar).
- Keyboard shortcuts everywhere, shown on the buttons and rows they trigger:
  G then a letter to move between places, J/K/Enter in lists, E, L, R, H on
  the selected item, Z to undo, ? for the full list, and ⌘K (Ctrl+K) to search
  every command. Shortcuts never fire while you type in a field.
- Storybook for the design system (`pnpm --dir clients storybook`), with the
  approved Home, Inbox, phone, states and themes screens on demo data, checked
  for accessibility in light and dark on every pull request.
- Versioned releases of Next: bumping `VERSION` (with a rolled changelog
  section) on `next` publishes `ghcr.io/notacamp/campbooks:next-vX.Y.Z` and
  `:next-vX.Y` (amd64 + arm64, Trivy-scanned, with provenance and SBOM
  attestations), tags `next-vX.Y.Z` and creates a GitHub prerelease. See
  CONTRIBUTING.md → Releases.
- Foundation for the rewrite: Rust API skeleton (kernel, AI gateway, `system`
  module with health and version), one React/TypeScript client with a Tauri 2
  shell, OpenSpec workflow, architecture decision records, CI quality and
  security gates, and docker compose for self-hosting.
- The server image now serves the web app next to the API on the same port:
  set `CAMPBOOKS_WEB_DIR` to a built client (the image does this for you).
  Deep links fall back to the app, hashed assets are cached for a year, and
  every response carries security headers (a strict CSP on pages).
- Self-hosting: `docker compose up` now also starts the identity provider
  (Rauthy) on port 8081, with no setup. Its secrets are generated on first boot
  into the new `idp-data` volume, and Campbooks' scopes and sign-in clients are
  registered automatically. Read the first admin password with
  `docker compose run --rm idp-init cat /app/data/campbooks/initial-admin-password`;
  see `deploy/README.md`.
- The API is now an OAuth 2.1 resource server. It validates JWT access tokens
  from your identity provider (signature against its JWKS, issuer, audience,
  expiry) and can require a scope on each endpoint. Rejected tokens get a 401,
  and valid tokens without the scope get a 403, both as problem+json with a
  `WWW-Authenticate` challenge. Self-hosters configure it with the new
  `CAMPBOOKS_AUTH__ISSUER`, `CAMPBOOKS_AUTH__AUDIENCE` (default
  `campbooks-api`), `CAMPBOOKS_AUTH__JWKS_URL` (discovered when unset) and
  `CAMPBOOKS_AUTH__LEEWAY_SECONDS` (default 60) settings. Without an issuer,
  scoped endpoints answer 401 `auth_not_configured`. Health and version stay
  public.
- `campbooks-server scopes` prints the API's scope catalogue as JSON, and the
  OpenAPI document declares a `bearer` security scheme and states every
  operation's security.
- Durable background work. Domain events are written to a retained event log
  in the same database transaction as the change they describe, and each
  module that subscribes gets its own delivery job, so nothing is lost on a
  restart. Jobs are retried with backoff, shared fairly between workspaces,
  handed to another worker if theirs dies, and failed jobs are kept for
  inspection. Everything lives in your Postgres (schema `kernel`); there is
  no broker to run.
- `campbooks-server` runs in roles: `all` (the API and the job worker in one
  process, the default and what the image runs), `api` and `worker`, so
  larger installs can scale the worker separately. `serve` still works as an
  alias of `all`.
- `campbooks-server events replay <consumer> [--since <time or event id>]`
  delivers logged events to one consumer again.
- New optional settings for the worker: `CAMPBOOKS_JOBS__CONCURRENCY`
  (default 4 jobs at once per process), `CAMPBOOKS_JOBS__PER_WORKSPACE`
  (2), `CAMPBOOKS_JOBS__POLL_INTERVAL_MS` (5000; new jobs also wake workers
  at once), `CAMPBOOKS_JOBS__LEASE_SECONDS` (60),
  `CAMPBOOKS_JOBS__RETENTION_HOURS` (168, how long succeeded jobs are kept),
  `CAMPBOOKS_JOBS__SHUTDOWN_GRACE_SECONDS` (8),
  `CAMPBOOKS_JOBS__RETRY_BASE_MS` (1000) and
  `CAMPBOOKS_JOBS__RETRY_MAX_SECONDS` (3600).
- Web sign-in. The web app now signs you in through the identity provider,
  and your browser never holds a token: the server keeps them (encrypted, in
  Postgres schema `bff`) and gives the browser a single `HttpOnly`,
  `SameSite=Strict` session cookie. Sessions end after 48 hours unused or 30
  days at most, on sign-out, or when you sign out at the identity provider
  (over https). Opening the app while signed out sends you to sign in and
  back to the same page. New settings: `CAMPBOOKS_SECRET_KEY` /
  `CAMPBOOKS_SECRET_KEY_FILE` (required once sign-in is configured; docker
  compose generates one for you into the new `server-secrets` volume) and the
  optional `CAMPBOOKS_BFF__*` block in `deploy/.env.example`. Changing the
  secret key signs everyone out of the web app and loses nothing else.
- `docker compose up` starts a one-shot `server-init` service that prepares the
  `server-secrets` volume; the web client's sign-in secret is now saved there
  (it used to be in `idp-data`).
- Storage for message bodies, attachments and files, outside Postgres. Each
  file is stored once per workspace however many messages carry it, checked
  against its SHA-256 on the way in and out, and deleted by an hourly sweep
  once nothing uses it and `CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS` (default 24)
  have passed. By default files go to the new `blobs-data` volume
  (`CAMPBOOKS_BLOBS__PATH`, `/data/blobs`); set
  `CAMPBOOKS_BLOBS__BACKEND=s3` and `CAMPBOOKS_BLOBS__S3__*` to use any
  S3-compatible bucket instead. Files over `CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES`
  (default 50 MiB) are refused. The server checks the store on startup and
  will not start if it cannot reach it. Back up `blobs-data` (or the bucket)
  together with Postgres; see `deploy/README.md`.
- The AI gateway's `decide` role is now typed (ADR 0009): modules ask
  `choice`, `score` and yes/no questions about an item and get answers with a
  probability per option and a confidence, which they must clear before
  acting. It is served by Laya, an open-weight decision model, or by any
  instruct model through the OpenAI-compatible API. New settings:
  `CAMPBOOKS_AI__DECIDE__PROTOCOL` (`laya` or `openai`, default `openai`, so
  existing `decide` settings keep working), `CAMPBOOKS_AI__DECIDE__LOGPROBS`
  (default `true`; set it to `false` for servers that reject the parameter)
  and `CAMPBOOKS_AI__DECIDE__CALIBRATION__<TYPE>[_<OPTIONS>]` temperatures
  (default 1.0). An empty `CAMPBOOKS_AI__<ROLE>__API_KEY` now means no key.
- Every API operation is now an entry of one catalogue, the OpenAPI document
  (ADR 0013): each has a stable `operationId` (`<module>.<verb>`, for example
  `system.health`; the old `system_health`-style ids are gone) and an
  `x-campbooks` extension saying whether it changes data, which operation
  undoes it, whether automation rules and the assistant may call it, its risk
  and the record types it touches. Domain events are listed the same way in
  the new `openapi/events.json` (`campbooks-server events catalogue` prints
  it).
- Mutating API calls accept an `Idempotency-Key` header: retrying a request
  with the same key returns the first response (marked
  `Idempotency-Replayed: true`) instead of running it twice. Reusing a key
  for a different request answers 422, and a key whose first request is still
  running answers 409. Keys are kept for 24 hours, in the new
  `kernel.idempotency` table (created by the migration that runs on startup),
  and an hourly job deletes expired ones.
- Self-hosting: `docker compose --profile ai` now also runs `laya`, the
  default `decide` model (Laya multilingual, Apache-2.0, CPU-friendly). It is
  built locally from `deploy/laya`, downloads its checkpoint (about 680 MB)
  into the new `laya-models` volume on first start, is reachable only on the
  compose network, and refuses to start until you set
  `CAMPBOOKS_AI__DECIDE__API_KEY` in `.env` (`openssl rand -hex 32`).
  `.env.example` now points `decide` at it; see `deploy/README.md`.
- Contributor tooling: `cargo xtask new-module <name>` scaffolds a module
  (crate, scopes, events, a scoped read slice with tests, schema migration,
  OpenSpec change, client module with its four locale files) and registers it
  in the workspace, the server and the client shell; `cargo xtask new-feature
<module> <feature>` adds a slice to a module; `cargo xtask scaffold-check`
  (also run in CI) proves their output passes the gates. Optional local git
  hooks (`lefthook install`), Claude Code skills for starting a task,
  developing a feature and adding a module, and a guide to working in
  parallel worktrees (`docs/parallel-work.md`).
- Workspaces. People who sign in get a Campbooks user; they create workspaces
  (business or personal, with an accent colour), invite others by email as
  admins or members, change roles, remove members or leave, and owners can
  delete a workspace: it disappears at once and its data is purged after 30
  days. A workspace always keeps at least one owner. Owners and admins can
  read a security log (sign-ins, role changes, invitations, removals,
  deletion). The API lives under `/api/v1/workspaces` and
  `/api/v1/w/{workspace_id}/…`, with new scopes `workspaces:read`,
  `workspaces:write` and `workspaces:admin` (registered in the bundled IdP;
  add them to sign-in clients provisioned earlier). Invitations are not
  emailed yet; set `CAMPBOOKS_ENV=development` to have the server log the
  link.
- `CAMPBOOKS_SIGNUP__POLICY` decides who may create a first workspace:
  `open` (default), `invite_only`, `approval` (requests are recorded and
  refused for now) or `code` (with `CAMPBOOKS_SIGNUP__CODES`). The instance's
  first user can always create one.
- Each workspace's data is also isolated by Postgres row-level security. The
  server now runs migrations as your database user and then queries as the
  role `campbooks_app` (created by the migrations; `CAMPBOOKS_DATABASE_ROLE`
  to change it), and warns at startup if that role would bypass the
  policies. Nothing to do with the bundled Postgres; on a managed Postgres
  whose user cannot create roles, see `deploy/README.md`.
- API errors for malformed JSON bodies and query strings are now
  problem+json 400s, and unparsable ids in paths are 404s.
- Monitoring, off unless you turn it on. `CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT`
  sends traces and metrics over OTLP/HTTP to a collector (with
  `CAMPBOOKS_TELEMETRY__OTLP_HEADERS` for its credentials and
  `CAMPBOOKS_TELEMETRY__SAMPLE_RATIO`, default 1, for the share of traces
  kept). `CAMPBOOKS_TELEMETRY__METRICS_ADDR` serves Prometheus metrics at
  `/metrics` on a separate port: keep it private. Metrics cover request
  rates, errors (by their stable `code`) and latency per route, job durations and failures, queue
  depth and wait, AI call latency and tokens per role, the database pool and
  the build. Traces follow a request into the jobs it enqueues and the calls
  it makes to AI providers and the identity provider, and honour an incoming
  W3C `traceparent`. No message content, token, cookie or email address is
  ever recorded; a test enforces it. See `deploy/README.md` → Monitoring.

### Changed

- Access tokens of the built-in issuer name `<CAMPBOOKS_PUBLIC_URL>/api` as their
  audience (RFC 8707), which the API now expects; `CAMPBOOKS_AUTH__AUDIENCE` only
  applies to an outside identity provider. Nothing to do: tokens last ten minutes, and
  the next refresh gets the new one.
- The token endpoint, client registration and the consent answer are rate-limited per
  client address (`CAMPBOOKS_TRUSTED_PROXIES` behind a reverse proxy).
- The members page's "copy invitation link" shortcut is now `Y` (`C` writes a
  new message everywhere).
- The web app's Content-Security-Policy now allows inline `style` attributes
  (`style-src-attr 'unsafe-inline'`) and `https:` images, which sanitised
  email bodies need inside their sandboxed frame; scripts, style sheets and
  connections stay same-origin. Reverse proxies that set their own CSP need
  the same two additions.
- Connecting a mailbox goes through the built-in sign-in: Google redirects to
  `/auth/callback/google`, and `/connections/google/callback` is no longer used
  (remove it from your Google OAuth client). `connectors.complete_connection` is
  gone; `connectors.start_connection` answers with the sign-in URL. An install still
  on Rauthy cannot connect mail until it switches to the built-in sign-in.
- **API clients:** `POST /api/v1/w/{workspace_id}/invitations` now answers
  with `accept_url`, the invitation's accept link, built from
  `CAMPBOOKS_PUBLIC_URL`. It is returned only in that response (never listed,
  logged or sent in an event); inviting the same address again returns a
  fresh link. Development servers no longer log invitation links.
- **API clients:** every problem+json error now carries a stable `code`
  (`not_found`, `invalid_token`, `last_owner`, …); `detail` is now a
  sentence in the caller's language instead of that code. Match on `code`.
- A user's saved language (`workspaces.users.locale`) is now empty until
  they choose one, so the browser's language applies meanwhile. The
  migration runs on startup; nothing to do.
- **Separate database logins.** The server now queries as a dedicated
  non-superuser login (`campbooks_runtime`) that cannot bypass row-level
  security or switch back to a more powerful role, and the owner login is used
  only to run migrations. New settings: `CAMPBOOKS_DATABASE_MIGRATION_URL`,
  `CAMPBOOKS_DATABASE_PASSWORD_FILE` and `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS`.
  **Upgrading with the bundled compose file needs no manual step**: the new
  `db-init` service creates the login and its password on the first
  `docker compose up -d` (existing data is untouched). With your own
  Postgres nothing breaks either: a single privileged `CAMPBOOKS_DATABASE_URL`
  keeps working, with a startup warning recommending the split (see "Database
  logins" in `deploy/README.md`). The server now refuses to start when the
  login it queries as is a superuser or has `BYPASSRLS` (for example with
  `CAMPBOOKS_DATABASE_ROLE=` set empty on a superuser URL); set
  `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS=true` to opt out.
- `/bff/api/*` now requires a signed-in session and calls the API with your
  access token; without a session it answers 401 `session_required`. It
  replaces the interim passthrough.
- Log lines inside a request or a job now carry the W3C `trace_id`, and the
  request's span records the route template (`http.route`) instead of the
  full URI, which could carry personal data in its query string. A
  client-sent `x-request-id` that is not a plain token is logged as
  `invalid`. The request log line is now `request finished`.
- The image's default command is now `all` instead of `serve` (same
  behaviour plus the job worker). Each job the worker runs may hold a
  database connection: keep `CAMPBOOKS_DATABASE_MAX_CONNECTIONS` (default 10)
  above `CAMPBOOKS_JOBS__CONCURRENCY`.
