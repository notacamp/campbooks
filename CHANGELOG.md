# Changelog

All notable changes to Campbooks Next are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Usage analytics, without a tracking script (off by default).** An instance can count
  which screens people use in its own Matomo (`CAMPBOOKS_ANALYTICS__MATOMO_URL`,
  `__MATOMO_SITE_ID` and `__MATOMO_VISITOR_KEY`, all three or nothing). The app loads no
  analytics script and its security policy is unchanged: after each navigation it tells
  its own server the screen's route template, such as `/w/:workspaceId/inbox`, never the
  address, an id, a search or a title, and it sends nothing while the browser asks not to
  be tracked (Do Not Track or Global Privacy Control). The server checks the template and
  passes it on in the background with the browser's user agent and a visitor id that
  changes every day, so Matomo sees daily visitors per screen but never an IP, an account
  or a workspace. A slow or unreachable Matomo only drops views; the app never waits for it.
- **Always show images from a sender you trust.** In the reader, beside Show images, "Always
  show images from <sender>" (⇧A) saves a smart rule, after the usual confirmation, for that
  address or anyone at its domain. Their images then show on their own, but only when the
  mail passes the sender's checks and Campbooks has found nothing suspicious about it: mail
  pretending to be them, mail failing DMARC, and mail with no checks keep their images held
  back. Tracking pixels stay removed either way. The rule itself is plain (no AI), but
  images wait for Scout's check of each email, so while Scout is off or past its daily
  limit they stay held back. Remove the rule in Scout's memory and Show images is back.
  The offer only appears where it can take effect (not for a sender already trusted, mail
  failing its checks, mail under review, or a provider that shares no sender checks), and
  otherwise the reader says in one line why images stay held back. "Anyone at" offers the
  sender's own domain, never a parent of it (a domain rule reaches its subdomains), and
  never a free mail or internet provider's domain (over 300 of them, subdomains included),
  a platform anyone can send from (Substack, Mailchimp, Google Groups and the like), a
  help desk whose customers send from its subdomains (Zendesk, Freshdesk) or an address
  relay (Hide My Email, SimpleLogin). The server refuses such a rule however it is made,
  and a rule like that stored earlier never acts: Scout's memory shows it paused as too
  broad, and the reader says why while the address alone may still be trusted. The wider
  free mail list also refuses domain-only rules of any kind for those providers' domains
  (trust an address instead). A second rule for a sender already covered, even by a
  paused one, is refused with a pointer to Scout's memory, also when a rule is confirmed
  or brought back. In a shared inbox the note says the inbox trusts the sender.
- **Home offers each email's own actions, picked by Scout (with the catalogue on).** When
  the action catalogue is switched on, the selected item's buttons are the ones the model
  ranked for that email, likelier first on 1, 2 and 3, with the visor saying Scout picked
  them. Remind asks when, Forward finds the person by name and opens the forward in the
  composer (nothing is sent from Home), and Open attachment opens the item on its file.
  Every action keeps its Undo. With the catalogue off, or for an email it didn't rate,
  Home offers the same actions as before.
- **Each item's own actions, from a catalogue (off for now).** The decide model can rate
  what Campbooks can do about each email (reply, remind, open the attachment, forward,
  report, it's safe, already paid or still to pay) in the same call that sorts it, and
  each need then carries its top actions, a bill's or a warning's own answers always
  first. The facts only rule an action out, and nothing reads what the email says. It
  stays off (`CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED`) until the owner has labelled
  their own mail and the floor is set from it: `campbooks-server labelling export --set
  actions` and `cargo xtask eval actions` are the tools for that. Off, nothing changes.
- **Smart rules, offered when they help.** After Report and delete, Home asks "Never see
  emails like this again?" and offers a rule for that sender's address. When the mail
  failed DMARC and its domain publishes a reject or quarantine policy, it offers instead
  to report mail pretending to be that domain, which never touches people you write to. A
  whole domain is never put in the trash or spam. After Already paid, it offers to mark
  that sender's invoices paid from now on. At the bottom of Home, "Scout noticed" suggests
  a rule for mail you get a lot of and rarely open, or archive unread, one at a time, with
  Not now and Don't suggest this. Every rule, teach once's included, opens the same
  confirmation first: the rule as a sentence, what it matches, and whether to apply it to
  the mail already here. Nothing is saved before Save smart rule, and Undo takes it back.
  Rules run on their own, with no AI, and Scout noticed reads only counts and dates, never
  what the mail says.
- **Trash and Report.** A conversation can go to the trash, or be reported as spam
  ("Report and delete"), with Undo for both. Untrash puts it back where it was. Not spam
  puts mail you received back in your inbox, as Gmail's own button does, and never your
  own sent mail. The provider is told too: Gmail through its own trash and spam, Zoho by
  moving the message to its Trash folder (and back to the folder it was in, when that
  folder still exists) or with Zoho's spam report. Taking a message out of spam at Zoho
  moves it to the Inbox but does not tell Zoho's spam filter. When Zoho can't make an undo
  (your own sent mail, say), the message shows in the trash or spam again after the sync.
  Gmail's untrash and Zoho's spam report work as their documentation says and still need
  checking on a live account. Mail in the trash or spam never needs you.

### Fixed

- **Gmail write-back stays within its quota budget.** Before writing your changes back to
  Gmail, a sync now reserves the most those writes can cost, not just what it plans: enough
  for Gmail to refuse a large batch (each message is then changed on its own) or for a
  change made meanwhile in Gmail to split a batch into smaller writes. Before, such pushes
  could spend far more than they took from the shared budget and run into Gmail's rate limit.
- **Mail whose HTML has no body tag gets its words.** Mail written as a head followed
  straight by its text, with no `<body>` (some mailers send it), was stored with a blank
  text in place of its words, so search found nothing in it, its preview and Scout's read
  were empty, and forwarding or replying to it quoted nothing. Its words are read from
  its HTML now, everywhere, and mail stored
  earlier is read again in the background after the update, the first time anyone
  searches or new mail arrives in a workspace; search keeps answering meanwhile.
- **The live feed resyncs whenever it finds an inbox nothing announced.** If an inbox ever
  became visible to someone without the change that should announce it, only the
  heartbeat's check noticed; a check triggered by a change to some other inbox let it in
  silently, and mail read past in the meantime was never sent. Every check now resyncs
  the client in that case, while an announced grant still needs none.
- **Desktop app: links stay where they belong.** On macOS and Linux the main window no
  longer treats `tauri.localhost` as the app (there it is the computer itself, where any
  local program could answer), only on Windows and Android, where the app is served from
  it. Mail links keep their spaces as `%20` instead of turning them into `+`, which mail
  apps showed as plus signs; files (`attach=`) are still never handed to the mail app,
  not even hidden after a `#` or inside another field's value.
- **Forward's picker stays fast on a big inbox.** Checking that no one it offers was
  reported or found suspicious now looks only at the senders it is about to show, through
  a new index, instead of reading a year of the inbox's mail on every keystroke (three
  indexes, built without blocking writes).
- **Home no longer keeps an older need when two changes race.** When a conversation's
  need changed twice in quick succession before it had an item on Home, two workers could
  both create the item and the older change could land last, leaving Home on the older
  need until the next change. The item now only ever moves to a newer revision. This was
  also why a replay test failed now and then in CI.
- **Home's Forward never offers a sender under review or a no-reply address.** The picker
  lists only people it is safe to write to: the server leaves out no-reply addresses,
  senders whose newest mail fails their sender checks or pretends to be them, and
  senders with any mail reported as spam, under review (even once a later email
  replaced the review) or found suspicious and not marked safe, from facts alone, never
  what the mail says. Only the flagged email's own sender is left out, so the real
  people in a hijacked thread stay. A name typed
  for one of them finds nobody. Closing Forward while it starts, then forwarding again,
  addresses the same draft rather than making a second one; a forward that fails while it
  starts no longer opens the composer after saying so; and Open attachment with no file
  says "This email has no file to open".
- **The labelling export says whom it is for.** Before reading any mail, `campbooks-server
  labelling export` prints the member it resolved (id and role) and the inboxes it will
  read, and records them in the folder's `.campbooks-labelling` marker.
- **The labelling export stays within one member's inboxes.** `campbooks-server labelling export`
  now requires `--member` (a member id or verified address) and exports only from the inboxes
  that member can read, never a teammate's private inbox; `--inbox` narrows it to one of them.
  The command prints where the folder is and how to delete it, and `campbooks-server labelling
  purge --out <folder>` removes a folder only if the export wrote it. A committed eval set can
  no longer claim `private` to skip its reserved-domain checks.
- **HTML mail looks right in the desktop app.** The desktop app now shows HTML mail as the web
  app does: styled, with its pictures, and remote images still held back until Show images or
  a trusted sender. Its page policy allows only inline style attributes and `https:`, `data:`
  and `blob:` images; nothing in a mail ever runs, posts or navigates. A link in a mail opens
  in your browser, never inside the app. Plain `http:` images stay blocked on desktop.
- **A signature logo is no attachment to open.** With the action catalogue on, an image
  shown in an email's text (a logo sent inline) no longer counts as an attachment, so
  Open attachment is never offered for mail that has nothing else attached.
- **One bad action answer never costs an email its sorting.** With the action catalogue
  on, an action answer the model leaves out or gets wrong (not a number, out of range)
  now just means no rating for that action; the email is sorted exactly as with the
  catalogue off. The sorting questions stay strict as before. The actions eval now asks
  each email what the model pass asks (Open attachment only when there is one), and the
  actions labelling export lists the message that raised each need.
- **Decide calls leave room to answer.** The OpenAI-compatible decide path now asks for up
  to 4096 tokens, so a reasoning model is never cut off before its answer. It is a cap: a
  model that answered before costs and takes the same.
- **Email HTML cleaning catches `URL(` in any case.** The sanitiser (ammonia 4.2.4,
  RUSTSEC-2026-0373) matched CSS `url(` case-sensitively, so a style written `URL("…")`
  could slip a remote image past the filter that holds images back until you choose
  Show images.

- **Scout's reply suggestions and drafts have room to think.** The model behind them
  (a reasoning model in production) spends its thinking from the same budget as its
  answer, and Reply gave it too little: suggestions and drafts could come back empty or cut
  short, and Reply would say Scout couldn't write one. Both now allow 4,096 tokens. When
  an answer still doesn't parse, the log says why the model stopped and how long the
  answer was, never what it wrote.
- **Sign in with Zoho works.** Signing in with Zoho, or connecting a Zoho mailbox,
  always ended on "The provider didn't let you in". Zoho publishes its signing key with
  an extra zero byte in front, and the server refused the key's every signature because
  of it. The server now reads the key without the zero byte and checks everything else as
  before.
- **Pay asks "Is it paid?", and receipts never land in Pay.** Pay used to be raised for
  any money mail the decide model read as asking something, so invoices already paid and
  payment confirmations showed up as "Pay". The decide model now answers one more
  question in the same call: does the email ask for a payment still owed, confirm one
  already made, or neither. Only a payment still owed raises Pay, and since Campbooks
  can't see your bank it asks on Home: "Acme Hosting sent an invoice. Is it paid?" with
  Already paid (1) and Still to pay (2), and "All N already paid" on the Pay group with one
  Undo. Still to pay keeps it as "Pay Acme Hosting", says how many days overdue it is when
  there's a due date, and offers a reminder first (two days before the due date, tomorrow
  morning when overdue, Monday with no date). The reader asks the same. Existing
  needs are worked out again on upgrade, and mail from the last two weeks is asked the new
  question once; a Pay item on older mail goes away and doesn't come back. A bill on
  recent mail stays until its question is answered, so one the upgrade could not ask (the
  daily AI limit, or a self-hosted model failing the answers check) is never dropped. The
  answers check now also requires the owed and paid cases to be right. Newsletters never
  raise Pay; other streams can, when the model is sure the payment is owed. After this
  upgrade, rolling back to an earlier image is not supported: the stored answers use the
  new money question and verdicts.
  For operators, `campbooks-server labelling export` writes your recent money mail as a
  sheet to label on your own machine, and `cargo xtask eval decide --set-dir` measures the
  model on it (campbooks-cloud#167).

### Added

- **`/up` works as a health check.** The server answers `/up` the same way as
  `/api/v1/system/health`, so load balancers and uptime monitors that probe `/up` work
  without being reconfigured.
- **Build a rule yourself, any time (campbooks-cloud#217).** In Scout's memory, "Build a
  rule yourself" next to the privacy line opens the rule builder without sending anything
  to a language model, and "Tell Scout in a sentence instead" goes back. When Scout can't
  make a rule of a sentence, it offers the builder too. In the builder, "It's from" finds
  senders by name, as in Scout's questions: type part of a name, or a full address or a
  domain. Each sender you tick becomes its own rule, so mail from any of them counts, and
  each rule shows what it matches before you save it.
- **Reply proposes Scout's draft.** Reply (R, an item's Reply on Home, the reader's Reply)
  now opens a dialog with two or three likely replies, each with its number key ("Yes,
  Thursday works", "Ask to move it to the afternoon"), a line to say it in your own words,
  and "Write my own" for the plain composer. Scout writes the draft only when you pick one,
  in the conversation's language and signed with your name. Edit it in place, make it
  shorter or ask for a change in plain words, then Send: it goes with the usual Undo
  window, the Home item folds away, and Undo brings the draft back. "Open in composer"
  moves it to the full composer. Nothing is sent until you press Send, and nothing is
  written before you pick. Servers with no `complete` model that may read mail offer only
  "Write my own", saying why. Items in an inbox you can only read no longer offer Reply,
  Done or Not needed on Home. Two new operations, `conversations.suggest_replies` and
  `conversations.write_reply`, need `conversations:send`, count against the workspace's
  daily AI limit and a per-member hourly allowance, and store nothing.


- **Scout's summary of the email on an opened Home item.** When an email raises a need, the
  `complete` model writes one or two plain sentences saying what it is about, once, in the
  background, and the opened item shows them first, under "Scout's summary" with the visor,
  above the email, so you can decide without reading it. The summary is written in all four
  languages in one call and you read yours. It is never written again and opening an item never
  asks the model. It counts against the workspace's daily AI limit (one call per email that
  raises a need) and appears only when a `complete` model is configured and may receive mail on
  this server; otherwise, or when the call fails, the email shows as before. Mail that may be
  phishing gets no summary, only its warning, so Scout never retells a scam in its own voice.
  Needs that were open before the upgrade get theirs in the background, with nothing to do.

- **Error reports to GlitchTip or Sentry.** Set `CAMPBOOKS_TELEMETRY__ERROR_DSN` and every
  internal error, job that failed for good and panic is reported, filed under the release
  and `CAMPBOOKS_TELEMETRY__ENVIRONMENT`. A report holds only ids (request, trace,
  workspace, job), the route or job kind, where it was logged and the kind of error, with
  quotes, addresses, links and tokens masked. Never the person, the request, a header, a
  cookie or the mail it was about. `campbooks-server telemetry test-error` sends one
  invented report to check the setup.
- **The app's own errors and page timings, for operators.** With
  `CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL` set, the app reports the errors it did not
  handle and its web vitals to its server, which scrubs them and forwards them to a Grafana
  Faro receiver. Only the route shown, a random id for that run of the app, the browser and
  the error's class, message head and stack positions are sent. Without the setting the app
  sends nothing.
- **Connector sync metrics.** `campbooks_connector_syncs_total` counts sync runs by provider
  and outcome, and `campbooks_connector_overdue_streams` shows streams waiting more than
  five minutes past due. `campbooks_errors_reported_total` and
  `campbooks_client_telemetry_items_total` count what reaches the tracker and the receiver.

### Changed

- **Web requests are logged and counted under the API route they ran.** A call the web app
  makes through the BFF (`/bff/api/*`) used to appear in the request log, its trace and the
  request metrics as `/bff/api/{*path}`; it now carries the API's route template
  (`GET /api/v1/w/{workspace_id}/members`) and `campbooks.via=bff`. Dashboards and alerts
  by `http_route` see the web app's traffic route by route.
- **An opened Home item shows the email (campbooks-cloud#203).** Pressing Enter on an item
  (or clicking the selected row) now shows the email it is about, rendered as the reader
  renders it, a few lines deep with "Show the whole email" to read it all in place. Until
  now an opened item showed only how it was decided. In mail that may be phishing, links
  lead nowhere, on Home and in the reader, and a line says so. Plain-text mail now keeps
  its line breaks and paragraphs instead of running together into one line. The selected item's
  background now covers its actions and what opening it shows, not only its first line.
- **Find people by name in Scout's questions.** In "Whose mail should always reach you?"
  and "What never needs you?", the address field is now a filter box. Type part of a name,
  an address or a domain and the cards narrow at once, then Scout finds anyone in your mail
  who matches, beyond the ten it suggests. The people you ticked stay listed. A full
  address or a domain no one matches still shows as its own card to tick. With text in the
  box, Enter ticks the first match and Esc clears it. "What never needs you?" no longer
  suggests the people you reply to most.
- **Settings pages are made of section cards.** Every Settings and Admin page now has a
  display-face title with its lead, then each section in its own card with a larger title,
  a count where it lists things ("Accounts · 2") and a one-line description. In Scout's
  memory the rules no longer sit under a second "Rules" heading, the sentence field keeps
  its label for screen readers only, and the privacy line has a lock. Ways to sign in
  gets an Accounts card with "Add a way to sign in" at its bottom and Sign out everywhere
  in its own card with an outlined danger button. Nothing behaves differently.

### Fixed

- **Teach Scout no longer blames your words when the model fails (campbooks-cloud#217).**
  A sentence like "anything from my accountants at northtax or atelier conseils always
  matters" could come back as "Scout couldn't turn that into a rule" when the model's
  answer was empty, cut short or wrapped in a code fence. Scout now gives a thinking
  model room to answer, reads an answer wrapped in a code fence, and when the answer
  still can't be read says Scout couldn't read the sentence just now, with Try again and
  "build the rule yourself". The server log says why the model stopped and how long its
  answer was, never what it wrote.
- **A taught rule that names a whole group of sites stops matching.** A rule saved with
  a sender domain like `github.io` (now known to cover every site under it) used to match
  all of them. It now matches nothing and shows as paused in Scout's memory, where you can
  remove it.
- **An unreadable `Authentication-Results` header counts as failed.** A sender can no longer
  hide a failed check by breaking the header with an unbalanced quote or comment. It only
  adds a hint for the decision model.

### Security

- **Scout can write a draft for you but never send it (campbooks-cloud#242).** When the
  assistant (or an automation) calls the API on your behalf, the server now refuses to
  send a draft (`send_needs_you`) or to take back a send you made, even with the token it
  read for you and even if the call was marked as confirmed: nothing is scheduled, nothing
  is signed and nothing reaches your mail provider. It may still write, change and discard
  drafts; you send them with your own Send. Before, sending was gated only by a "confirmed"
  flag that the calling code sets itself. The build now refuses any high-risk operation
  that the assistant or an automation could call. The assistant can also no longer save,
  reshape or bring back a rule that shows a sender's images (`images_need_you`), so a
  suggestion of its own can't stand in the way of your "Always show images" for that
  sender. No Scout feature calls any of these on its own today: this closes the door
  before one does. The Scout reply dialog now also discards the draft it replaces when
  that draft's send had failed, not only while it was a draft.
- **Scout can propose rules but never keep them for you (campbooks-cloud#242).** When the
  assistant (or an automation) calls the API on your behalf, the server now refuses (`rule_needs_your_yes`) to
  say yes to a rule, to adopt one, to let a paused one act again, to change what a kept
  rule does, or to bring a removed rule back acting, even with the token it read for you
  and even if the call was marked as confirmed: nothing is signed and nothing changes. A
  rule it saves is always a suggestion waiting for your yes in Scout's memory, whatever it
  asked for and even if you let Campbooks act without asking. It may still pause, remove
  or turn down a rule, and change a suggestion before you see it. Your own Keep works as
  before. Which requests come from the assistant is decided inside the server and cannot
  be set by an app. A yes to a suggestion now makes you its author, as a yes again already
  did, so it acts with your rights rather than those of whoever it was stored under. A
  message that is scheduled to send can no longer be discarded, only taken back with Undo
  (then it can be deleted), and the Scout reply dialog never discards a message on its way.
  No Scout feature in this release calls these on its own yet: this closes the door before
  one does.
- **Send, saves and Keep act only on what you were shown (campbooks-cloud#242).** Every
  draft and rule the server shows you now comes with a token bound to you and to exactly
  that draft or rule: a draft's recipients (Bcc too), subject, text, quoted text, reply
  headers, inbox and attachments at its version, or a rule's conditions, actions, inbox,
  state and author. Every save of a draft, Send, a yes to a suggested rule and adopting a
  rule take that token back, and the server acts only if the row is still what your app
  read; each save hands back the token of what it saved. A draft or suggestion changed with
  SQL after your app read it, or a token of another draft, rule or person, is refused
  (`content_changed`, a warning in the log with ids only): nothing is saved, sent or kept.
  The composer keeps what you typed, reads the draft again and says "This changed since you
  opened it" once; a refused Send shows the draft as it now is, Bcc included. Scout's reply
  dialog no longer reuses a draft it didn't create: it writes a fresh one with only the
  recipients it shows, no copies and no attachments. What changed before your app first
  read a draft is in what it shows you; fields no screen shows (reply headers, the file
  behind an attachment's name) are not checked at that point. An editor open at the
  upgrade shows the notice once, keeps your text, and works on the next save or Send.
  Scheduled sends are not affected.
- **A planted rule run or undo changes nothing (campbooks-cloud#242).** Applying a rule to
  the mail already in your inbox, and removing a rule while putting back what it changed,
  now sign the run for the member who asked, under the key of campbooks-cloud#227, and each
  change a rule records about a conversation is signed where the rule makes it, over what it
  was before and what the rule did. A run written with SQL, or one copied from another run,
  stops before its first change (a warning in the log with ids only), and putting a rule's
  changes back skips any record that was written or edited with SQL, so it can no longer take
  mail out of spam or the trash at Gmail or Zoho; the rest is still put back. A run of a rule
  that has been changed, or paused and resumed, since it started stops too. Runs going
  during the upgrade stop: apply the rule again if you need to. What a rule changed before the
  upgrade can't be put back automatically: removing such a rule while putting its changes
  back answers `undo_unavailable`, and removing it keeping its changes still works.
- **A taught rule planted or changed with SQL changes nothing (campbooks-cloud#242).**
  Saving, confirming, changing, pausing, removing or bringing back a rule now signs it for
  the member who did it, under the key of campbooks-cloud#227, over its conditions,
  actions, inbox, author, state and precedence. A rule acts on arriving mail, on the mail
  already in the inbox and on "Always show images" only while that signature holds, so a
  rule written or edited with SQL is skipped (a warning in the log with ids only), asks
  Gmail or Zoho for nothing, and the other rules still act. Arriving mail is never held up
  by a bad rule. Such a rule is listed as needing a yes again (`needs_confirmation`), and
  confirming it signs it as it is. The rules you have when you upgrade keep working: the
  migration records a fingerprint of each one, and a job signs each rule once, only if it
  is still exactly as the migration found it. A rule changed in between waits for you to
  confirm it. Rules don't act for the first minute or so after the upgrade, until that job
  has run, and without `CAMPBOOKS_SECRET_KEY` a rule can't be saved active.
- **SQL alone can't make a mailbox send (campbooks-cloud#227).** Sending a
  draft now signs the author's request, under a key derived from the instance secret that
  never goes in the database, over the draft, its send, the author, the inbox and
  everything that goes out: recipients, subject, body, reply headers and attachments. A
  draft is released only while that signature still holds, so a draft planted or edited
  with SQL (other recipients, another body, another author) is never sent and is left as
  it is, with a warning in the log and no content. The released send is signed in turn
  and checked before every call to Gmail or Zoho, and so are change requests and each
  change the connector records, before it is made at the provider; one that doesn't hold
  is skipped and the provider's state comes back. Changes made by taught rules are
  still signed by the server, so a rule planted with SQL can still change mail; that is
  campbooks-cloud#242. Without `CAMPBOOKS_SECRET_KEY` nothing
  is sent (it was already needed to connect a mailbox). Two migrations add the signatures;
  drafts waiting in their undo window during the upgrade go back to drafts, to be sent
  again, and a change pending during the upgrade isn't made.
- **An event alone can't make a mailbox send or change mail (campbooks-cloud#226).**
  The connector no longer takes a send or change request event as proof. Before it records
  a send, and again before every call to Gmail or Zoho for it, it asks the conversations
  module whether the send still stands: the draft is being sent as that send, from that
  inbox, by an author who may still send it. It sends the message that module recorded
  when the draft left its undo window, never one an event names, and only from the
  mailbox of the draft's own inbox. Each call to the provider is claimed first, so two
  jobs for one send never both send it. A change (read, star, archive, a label, a stream)
  is made only when the conversations module recorded that very request. A request that
  doesn't stand does nothing and is logged, without content. Someone who can run SQL in a
  workspace can still release a draft of their own; closing that is campbooks-cloud#227.
  Two conversations migrations record the outgoing document with each send and every
  change request. A send or change made by an older image during the upgrade isn't made:
  its draft shows as failed, at once or after an hour, and can be sent again. To go back to an earlier
  image, run the rollback SQL in the change's `design.md` first.
- **A workspace can't plant maintenance jobs, fake a disconnect or read account events
  (campbooks-cloud#211).** The worker now refuses, without running it, a job of a kind that
  only ever runs instance-wide (every schedule, such as pruning and the mailbox poller, and
  the deliveries of account events) when the row carries a workspace; such a row could
  only have been written by hand. Six handlers no longer take an event from a workspace
  as proof of what it says: a way in goes with its mailbox only once the connectors module
  no longer holds that connection, a mailbox record is named only with a connection that
  exists, and a departed member's app consents and provider grants, or a deleted
  workspace's, are revoked only once the member is really gone or the workspace really
  deleted. A transaction working in a workspace no longer reads any instance-wide row of
  the event log or the job queue (they name accounts); the kernel publishes the account
  events a workspace may announce without reading them back. A schedule's tick is counted
  by the statement's clock, so a stalled tick can't move a newer one back. One kernel
  migration drops the read policy; an image from before this change can't announce those
  account events from a workspace on the upgraded database, so run the API and the worker
  of the same version, and to go back run the rollback SQL in the change's `design.md`
  first.
- **One workspace can no longer hold up another's background work (campbooks-cloud#209).**
  The job queue's keys spanned workspaces: a workspace that knew another workspace's job
  key (most embed an id, such as `connectors.sync_stream:<stream>`) could keep a row of its
  own with that key, and the other workspace's next enqueue was dropped as a duplicate, or
  waited behind it for ever. A mailbox's sync could stop that way. Unique keys and ordering
  keys now count per workspace, with instance-wide work (schedules, account events) as one
  more group, so it still runs once per tick. Three kernel migrations swap the indexes,
  building the new ones concurrently. An image from before this change can no longer
  enqueue keyed jobs on the upgraded database: run the API and the worker of the same
  version, and to go back run the rollback SQL in the change's `design.md` first.
- **Catching up on changes no longer times what you may not see.** A stream that is
  behind (after reconnecting, or after you stopped reading for a while) now sends one page
  of changes per tick, each a tick after you took the previous one, so when a page arrives
  no longer depends on how many changes it skipped in inboxes you are not in. A page whose
  read takes longer than a tick still arrives a whole tick later. An inbox you were given
  without notice now makes your stream start over at its next heartbeat instead of missing
  that inbox's first changes. Set the tick with `CAMPBOOKS_CHANGES__CATCH_UP_TICK_MS`
  (50 ms by default, 50 to 1000). Live changes still arrive at once.
- **A stream lists your inboxes once, and again only when access changes.** It used to
  list them on every wake-up. Now it keeps the list until someone's access to an inbox
  changes, or until its next heartbeat. When you are given an inbox (connecting a
  mailbox), its first mail still reaches open streams at once. When your membership ends,
  your stream ends at once, even if you could not see the change that removed you.
- **A message built to be slow to parse no longer ties up the server.** HTML made of
  thousands of stray or unclosed tags could take minutes of CPU every time the message was
  read, and again when search indexed it. The reader now measures the parse first and shows
  such a message as plain text once it passes a fixed budget, and search reduces HTML to
  text without building a tree. Ordinary mail is unchanged. No migration
  (campbooks-cloud#205).
- **A suspension and its lift always reach a member's rules in the right order.** Each
  account now counts its suspensions and lifts, and the rules a member taught follow the
  latest count, not the clock: a clock that stepped back can no longer leave them paused
  after a lift. A member who joins or leaves a workspace at the moment they are suspended
  or lifted now gets the right state there too. If a suspension fails to reach the rules,
  Admin → Jobs shows it under "Work to watch" and the server logs "suspension fan-out
  delivery died". Keeping a suggested rule whose author is away now says first that it
  stays paused until they are back or someone takes it over.
- **The database functions that act for the server no longer run as a superuser.** The
  functions behind web sign-in and the rate limits run with their owner's rights. Their
  owner was the login that applied the migrations, which on the bundled Postgres is a
  superuser, so a bug in one of them would have had every right on the server. They now
  belong to `campbooks_definer`, a role that cannot log in, owns no table and holds only
  the few privileges those functions use. The migrations create it. If your migration
  login is neither a superuser nor allowed to create roles, the upgrade stops and names
  the two statements an administrator must run once (see `deploy/README.md`). The
  functions also check their input more strictly: a sign-in can only land on a path of
  this site, as the server already checks. `CAMPBOOKS_BFF__ABSOLUTE_TIMEOUT_HOURS` must
  now be at most 8784 (366 days); the server refuses to start otherwise. Signing in again
  in the same browser now also revokes the earlier session's refresh token. Nobody is
  signed out by the upgrade. If you roll back, run the SQL in the change's design first
  (campbooks-cloud#214).
- **Web sign-in records are reachable only through the sign-in's own functions.** The
  tables holding web sessions, sign-ins in progress and retired session tokens could be
  read and changed by any of the server's database work. Now the server's database role
  has no access to them at all. It goes through narrow functions, each acting on the one
  session whose cookie a request presented, the one sign-in it is finishing, or one
  person's sessions; only the clean-up of ended sessions reaches across people. A sign-in's
  nonce, the one value that was stored in plain text, is now encrypted like the rest.
  Nobody is signed out by the upgrade. A sign-in started in the ten minutes before it
  answers "invalid state" once; start it again. Upgrade the server and the worker
  together: a process of the previous version fails every web request until it is
  replaced. If you roll back, run the SQL in the change's design first
  (campbooks-cloud#212).
- **Smaller fixes from the same review.** `CAMPBOOKS_CHANGES__OPENS_WINDOW_SECONDS` must
  now be at most a day (a longer one made every change stream fail to open); the server
  refuses to start otherwise. The rate-limit clean-up no longer deletes a counter renewed
  at the same moment. Migrations may pin a function's `search_path` only to exactly
  `pg_catalog, pg_temp` (campbooks-cloud#213).
- **Retried requests never bring back a secret, and keys and counters stay private.**
  Requests sent with an `Idempotency-Key` used to have their answers cached for a day,
  including a new app's client secret. That cache could be read from any workspace's work,
  and anyone could reset or use up the rate-limit counters. Now each caller reaches only
  their own keys. The answers that show something once (an app's client secret, a rotated
  secret, an invitation link, a link or connection ticket) are never kept. Sending such a
  request again with the same key answers 409 `idempotency_secret_shown_once` instead of
  showing the secret twice. Answers of those requests cached before this release are
  dropped. Rate-limit counters can only be counted and pruned through two database
  functions. No connection can read, reset or rewrite them.
- **A workspace can no longer forge background work for the whole instance.** Work running
  in a workspace could add a job or event that belongs to no workspace. One such row could
  stop the job pruning, or all mail polling, for good. It can now only add the account
  events a workspace announces (an app's access revoked, a way to sign in removed) and their
  deliveries, and no row of its own may hold a schedule's key. Nor can it touch the
  schedules' record of their last runs any more, and an account's event it writes into its
  own workspace by hand (to sign someone out everywhere, say) is never acted on. A schedule
  blocked by a row
  that will never run is freed at its next tick, the old row kept as failed. Administrators
  with `jobs.retry` can also discard a schedule's pending or stuck run. The mail and needs
  dispatchers no longer leave the last workspace set after they queue work, and the
  architecture checks catch scope names spelled with escapes and test modules hidden inside
  other code (campbooks-cloud#207). Rolling back to an older image needs a one-off SQL step,
  in the change's design.
- **Background work shared by the whole instance can only be changed by the kernel.**
  Account events, account jobs and scheduled ticks belong to no workspace. Work running in
  a workspace could still read and add them, which it needs, but it could also complete,
  requeue or delete them. Now only the kernel's own machinery (the worker, retention, the
  admin's queue) can change them. The two schedulers that work across workspaces (polling
  mailboxes, the needs backfill) now get a narrower scope that reads only their own
  schedule and reaches no other workspace's events or jobs. The architecture check also
  closes four ways around it (campbooks-cloud#196). Before redeploying an older version,
  run the rollback in the change's design notes.
- **The change feed reveals nothing about inboxes you can't see.** The live stream of
  what changed used to carry the workspace's own change count: as the `ready` frame's
  `head`, as every frame's cursor (so the gap between two of your changes counted the
  ones you were not shown), in a heartbeat sent only when hidden changes had happened,
  in when a resume answered `too_far_behind` or `cursor_expired`, and in how a catch-up
  came in bursts (a page was 200 changes of the workspace, hidden ones included). A
  member outside an inbox could tell from it how much mail that inbox gets, and when.
  The stream now reads pages of changes you may see only, filtered in the database;
  cursors are sealed resume tokens, new on every frame, all the same length and good
  only for you; `ready` has no `head`; heartbeats come on a fixed timer, busy or idle,
  each with a fresh token; every change you may see is its own frame; and a token
  expires by its age alone (changes are now kept for the retention period after they
  were numbered). A test compares each kind of caller's stream (member, admin, a client
  acting for a member, a client acting for nobody) with and without activity they may
  not see, and they match. `CAMPBOOKS_CHANGES__MAX_BACKLOG` and the `too_far_behind`
  reset are gone; opening streams is now limited per person or client
  (`CAMPBOOKS_CHANGES__OPENS_PER_WINDOW`, 30 per `OPENS_WINDOW_SECONDS` 60 by default),
  and the server warns at startup when `CAMPBOOKS_SECRET_KEY` is not set. Apps holding
  a cursor from before refetch once. Migration
  `20261020220000_kernel__change_sequenced_at` (campbooks-cloud#131).
- **A suspended member's rules pause until the suspension is lifted.** A rule acts in
  its author's name, so while an administrator has the author's account suspended their
  rules read as paused, exactly as when the author can no longer change the inbox: they
  leave new mail alone, a run over existing mail stops at its next batch, and previews
  and suggestions no longer count them. Unsuspending makes them act again, with nothing
  to redo (a run they stopped is not restarted). Other members see only that the rule
  is paused for now and acts again on its own, never that its author is suspended. A
  member removed while suspended gets their rules back when invited again. The identity
  module now also publishes `identity.account_unsuspended`, both suspension events carry
  an ordered `at`, and the workspaces module announces each in every workspace of the
  account (`workspaces.member_suspended` / `workspaces.member_unsuspended`). Migration
  `20261020210000_conversations__member_suspensions`. An account suspended before this
  version keeps its rules acting until an administrator unsuspends and suspends it again
  (campbooks-cloud#147).
- **A database transaction that names no workspace now sees nothing.** The kernel's
  event log, job queue, change feed and blob catalogue used to show every workspace's
  rows to any transaction that set no workspace, which is how background work crossed
  workspaces, but also meant code that forgot to pick a workspace could read and write
  every tenant's. Cross-workspace work now needs an explicit kernel scope that only the
  kernel opens (and, by a reviewed exception in `architecture.toml`, the two schedulers'
  dispatchers); everything else sees no row of any table with row-level security.
  Enqueueing a workspace's job, recording a change of the feed and attaching or
  detaching a blob now fail in a transaction that cannot reach that workspace, instead
  of silently doing nothing, and `cargo xtask arch` also checks `acquire()`, SQL kept in
  constants, and the server and kernel code. Migrations
  `20261020200000_kernel__kernel_scope`, `20261020200100_connectors__kernel_scope` and
  `20261020200200_conversations__kernel_scope` replace the policies; nothing to do on
  upgrade. **Upgrade every process at once**: if you run the API and the job worker as
  separate containers (or several replicas), stop the old ones before or as the new ones
  start, because a process still on an older version stops claiming jobs and cannot sign
  anyone in once the migrations have run. Going back to an older version needs a SQL
  rollback first (the policies' revert is in the change's design notes,
  `openspec/changes/archive/2026-10-16-kernel-scope/design.md`, "Rollback").
- **Sender authentication is read more strictly.** An aligned DKIM signature no longer
  counts when the domain's DMARC policy refused the mail (`fail`, `quarantine`, `reject`),
  an authentication header over 8 KiB is treated as a failure instead of being parsed, and
  "same organisation" now follows the real Public Suffix List (so two `github.io` sites, or
  suffixes such as `co.at`, are never one sender) instead of a 24-entry guess. Separately,
  a message that is being moved to spam or the trash can no longer have a first "needs you"
  stored for it by a refresh running at the same moment. No migration.
- **The API is rate-limited per person and per app.** Every call with a token now counts
  against a limit for whoever makes it, however it arrives (directly, through the web app,
  or from automation and the assistant): 600 reads and 120 writes a minute per person and
  app, 30 mail sends per 10 minutes per person across all their apps, and 120 admin API
  calls a minute. Over a limit the answer is 429 `rate_limited` with `Retry-After`, in the
  caller's language. Automation and the assistant are counted apart from the person's own
  apps, so a looping rule stops without locking the person out. Reads, writes and admin
  calls are counted in memory per server process; sends in Postgres (no migration). New
  settings `CAMPBOOKS_RATE_LIMITS__*` (`ENABLED`, and per class `READS`, `WRITES`, `SENDS`,
  `ADMIN` with their `…_WINDOW_SECONDS`; `0` turns a class off). `x-campbooks` gains
  `rate_limit: send`, declared by `conversations.send_draft`.
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
- **The rules list says why a rule is paused**, first on its line: its author can no
  longer change the inbox, its author is away for now (it acts again on its own; the
  cause is never shown), a member paused it, or the inbox is kept read-only
  (`RuleView.paused_reason`). A rule a member paused no longer claims its author lost
  access.
- One "Search or ask" field and menu, with conversations inline. The sidebar's
  separate "Search conversations" entry is gone: ⌘K and `/` (on screens 48rem and
  wider) open the same menu, which lists matching conversations first (matched words
  marked, the filters it understood as removable chips), then commands, then Ask,
  under one cursor: ↑ ↓ move, ↵ opens, ⌘↵ shows all results. On a phone `/` and the
  top bar's search button still open the results page.
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
- **New features no longer stay hidden from people signed in before the release that added them.**
  A web session kept the permissions it asked for at sign-in, so after a release that adds
  some (Scout's questions and memory in this one), the new features answered 403 and showed
  nothing until the person signed out. Now a session that predates the permissions the web app
  asks for ends on its next request and the app asks you to sign in again, which goes straight
  through while your sign-in at the identity provider is still live; signing out of such a
  session still ends everything. Migration `20261020230000_bff__session_scopes` adds two
  nullable columns. After upgrading, everyone signed in to the web app signs in once more.
  (campbooks-cloud#172)
- **HTML email renders with its own styles, safely (campbooks-cloud#160).** Newsletters,
  receipts and Outlook or Gmail replies looked broken: their style sheets were dropped
  (the page's policy blocks them in the mail frame too), the document title and hidden
  preview text showed up in the body, the frame's height was a guess (mail was cut short
  or padded, and long mail scrolled inside its frame) and a 600px layout did not fit a
  phone. Now the server inlines a message's style sheets into `style` attributes before
  sanitising (never fetching a linked or imported sheet), filters every declaration with a
  real CSS parser (nothing that loads or runs survives: no `url()`, `@import`,
  `expression()`, `var()`, escapes), and returns the message's `@media` rules, filtered the
  same way, in a new `css` field of the message body. The reader's frame is as tall as its
  content, shrinks a layout wider than the reader to fit, and applies the `@media` rules to
  the frame only. It still never runs a script, submits a form or loads a remote image
  before "Show images"; tracking pixels sized or hidden by their style are now removed
  too. The page's own policy is unchanged.
- **A need settled while its follow-up waited is no longer overwritten.** When a first
  "needs you" was put off because its message was busy, the follow-up could store it from
  what it had read before waiting: an open need on a conversation archived meanwhile, or
  over a Done a member had just given. It now works the need out again once it has its
  locks. No migration.
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
- **Home fits the window on desktop too, and scrollbars match the app.** A busy Home
  still made the whole page scroll and pushed the account menu down; from 1024px wide
  Home now scrolls on its own beside the sidebar, like the inbox. Scrollbars are thin
  and quiet, a soft thumb with no track that firms up when you point at the pane, in
  light and dark.
- **The inbox no longer grows taller than the window on desktop.** A long list of
  conversations made the whole page scroll, pushing the account menu down and taking
  the open conversation with it. From 1024px wide the page is the window's height:
  the sidebar, the list and the conversation each scroll on their own, and the list's
  header and the conversation's toolbar stay at the top. Settings and administration
  scroll the same way. Phones are unchanged.
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

- **Home: every item says what you can do with it, right where you look.** The selected
  item shows its actions under it, in one button style with each key: its own actions on
  1 to 3 (Reply; Yes and No for a question; It's safe for a warning), then Done (E),
  Snooze… (S) and Not needed (X). Other rows stay calm, with no buttons or bare key hints,
  and the keyboard legend leaves the aside (every key is in the `?` sheet). ↑/↓ move
  between items as J and K do, ←/→ move a highlight along the buttons, Enter presses it
  and Esc clears it. An item you act on turns into a one-line confirmation ("Done · Marta
  Ferreira") and folds away in under a second; the toast keeps Undo and Z
  (campbooks-cloud#165).
- **Snooze, in a dialog.** Later is now Snooze, on S: a dialog offers Later today,
  Tomorrow morning, Monday morning and the new Next week, each with its number key; Esc
  closes it with nothing changed, and on a phone it is a bottom sheet. It is the new
  `lib/ui` Dialog, which the reply and smart-rule dialogs will reuse, with a result state
  that closes by itself.
- **The toast can wait and can stay.** Actions that take a moment can show "Sending…"
  with the visor thinking (and Undo while it can still be taken back), and a failure now
  stays until you choose Retry or dismiss it instead of fading after six seconds.
- **Pin, on P.** Star is now called Pin, and pinned conversations stay at the top of
  their stream in the inbox. It is still the provider's star (Gmail's STARRED), synced
  as before. Migrations `20261020235000_conversations__pinned_stream_list_index` and
  `20261020235100_conversations__pinned_inbox_list_index` add the matching indexes,
  built concurrently.

- **A daily AI limit per workspace (`CAMPBOOKS_AI__DAILY_CALLS_PER_WORKSPACE`, default
  5000, `0` for none).** Every model call (`decide`, `embed`, `complete`, `vision`) is
  now counted per workspace, role and feature, per UTC day, with the tokens the
  provider reports, in two new tables of the `system` schema (migration
  `20261020190000_system__ai_usage`). Past the limit a workspace's new mail is still
  stored and sorted by the facts and taught rules, without the model pass, until
  00:00 UTC; nothing errors, and skipped mail is not asked again later. One log line
  says when a workspace reaches it. If the count cannot be written, the call is not
  made and the job retries. A tenth of the limit is kept for what people ask
  themselves: background work stops 500 calls short of the default 5000, so a mail
  flood cannot silence Scout; past the whole limit, compiling a sentence answers 503
  `ai_daily_limit_reached`. Administrators read a day's usage per workspace, capped
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
- **Tell Scout what you care about** (campbooks-cloud#121): three optional
  questions offered on Home after mail is connected (whose mail always
  reaches you, what never needs you, and whether Scout should ask first),
  and a "Teach Scout" box in Scout's memory. Answers and sentences become
  rules you check before saving; saved rules run without AI. Teaching in
  your own words uses the `complete` AI role only when you press Enter, and
  sends it only your sentence and your own addresses and label names, never
  your mail. Without a `complete` role, Scout's memory offers a rule builder
  instead. Scout's memory is a Settings section (Settings → Scout's memory)
  and lists your rules. If saving stops partway, Try again saves only what
  is missing, and the count you see is what was saved. Keys: `T` / `Shift+T`
  on the Home offer, `B` to go back, `Q` in Scout's memory, `⌘↵` to save a
  rule. New migrations: the `assistant` schema, and an index on received
  mail built without blocking writes; nothing to do.
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
