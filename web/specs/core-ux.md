# Spec — core UX (the assistant-first shell)

The locked model (2026-09-21). One assistant over three familiar tools. Build to the design
system in [`../DESIGN.md`](../DESIGN.md) + [`../tokens.css`](../tokens.css). Interactive
reference (demo data, same tokens): https://claude.ai/artifact/KSbgiWXxBqDfscYgqAtjd2

> **Why:** the five-place model (Now/People/Paper/Money/Time) + its vocabulary (streams,
> standings, lanes, asks, skim decks) made the user do the organizing the AI was meant to do.
> This puts the assistant in charge and uses tools people already recognize. The Track
> issue-list is retired.

Ownership: this spec + design + review = `campbooks-b4` (Opus authors, Sonnet builds, b4 reviews).
Scaffold/plumbing = `campbooks-1a`. `/api/app` = `campbooks-e1`.

## Shell

`Sidebar (≈220px) | main`, with **Scout as a persistent bar** pinned to the bottom of the main
area on every surface (an input + send, ⌘K to focus; submitting opens an answer popover above
the bar). Below 680px the sidebar becomes a 4-item bottom tab bar and Scout sits above it.

- **Sidebar:** workspace mark (wordmark + Visor); four nav items, each `icon · label · count
  badge` — **Today, Inbox, Books, Calendar**; foot = Scout status ("· Mistral, EU", Visor goes
  `asleep` when AI is paused) + the user (Avatar + name).
- **Router:** `/today` (default), `/inbox`, `/books`, `/calendar`. Scout is not a route.

## Today  (the assistant's desk)

The home. Replaces the infinite feed with a **finite, ranked worklist that empties**.
1. Greeting + **Scout brief** (Visor + one paragraph: what it read, what it filed, how many
   things need you, how many deadlines).
2. **Needs you** — a ranked list of items (reply / pay / decide / chase…). Each: Avatar,
   title ("Reply to Sofia"), sub, Scout's one-line read (why it matters + deadline), a Signal
   pill for the verb, and inline actions (primary + secondaries). Acting resolves it **in
   place with an Undo** (see DESIGN.md "Feedback, not toasts") and it collapses out. Cleared →
   a calm "You're clear for today" with the `happy` Visor.
3. **Coming up** — deadlines/reminders this week (date · what · a pill). Soon = `now-text`.
4. **Handled since <last visit>** — a quiet line (N filed · N matched · N tucked away) + the
   trust line "Nothing was sent, paid or deleted without you."
- **Skim entry:** a "Skim these" button on the Needs-you header launches Skim over the worklist.

**Data — the ONE net-new read (e1 builds pending their user's greenlight).** e1's `/api/app/now`
(now_deck) is an infinite timeline + segment rings, not this finite-worklist model. Today needs a
small aggregator over data that already exists (`People::Attention` + `Money::Evidence` needs-you
+ `Time` asks/reminders). Proposed, read-only — each item's action routes back to the **existing**
People/Money/Time mutations (no new writes):
```jsonc
GET /api/app/today -> { data: {
  greeting: { name, date, brief },                    // Scout's paragraph
  needs_you: [ {                                       // ranked; empties as actioned
    id, source: "people"|"money"|"time", ref_id,       // routes the action to the existing endpoint
    verb: "reply"|"pay"|"decide"|"do"|"chase"|"nudge",
    place: "people"|"money"|"time"|"now",
    title, subtitle, read /* =stand_line */, due /* iso|null */, draft /* bool */,
    actions: [ { kind, label, primary /* bool */ } ]   // kind -> the source's action endpoint
  } ],
  coming_up: [ { on /* iso */, label, sub, place } ],   // deadlines/reminders this week
  handled: { filed, matched, tucked, added, since }     // the quiet trust-line counts
} }
```
**Built & green (e1):** serializer `app/serializers/api/app/today_serializer.rb`, aggregator
`app/services/today/aggregator.rb`, spec `spec/requests/api/app/today_spec.rb` (14 ex). Adds an
`overdue` bool per item; ranking = pay→decide→chase→reply→nudge→do (overdue first, then due asc);
`coming_up` = next 3 days, ≤5. Money items are omitted (not 403) when accounting is off.

## Inbox  (unified, familiar)  ← the pilot surface

One mailbox, recognizable as email, AI-sorted and annotated. See [`inbox-pilot.md`](./inbox-pilot.md)
for the buildable first slice + the authoritative `/api/app` contract.
- **Search** bar on top ("Search all mail, people and documents").
- **Important / Everything** tabs (AI decides Important). List rows: Avatar, sender + time,
  subject, and a plain-language **annotation** ("Wants a reply · by Thursday", "Invoice · €148
  due Friday", "Newsletter · tucked away") with a signal dot. Unread dot in the accent.
- **Reading pane** (≥900px; a route/sheet below): sender header, a **Scout "what this is"**
  note (what it is + what it wants), the thread body, attachments inline ("extracted to Books"),
  and actions (Use Scout's draft / Reply / Archive).
- **Skim entry:** "Skim the long tail" over Everything (fast Keep / Archive / Approve).
- Retires: streams, standings, lanes, sender-type taxonomy. Documents (old "Paper") live here as
  attachments — extracted, tracked, searchable; financial docs also surface in Books.

## Books  (accounting)

A real ledger. Not a hero-metric grid; one cash line + working lists.
- **Cash line:** cash on hand + a "+/− this month" delta + "N of M lines reconciled".
- **Needs you:** reconciliation lines lacking a document / a confirm / a payment (each with a
  small action: Hunt / Confirm / Mark paid).
- **Reconciled this month:** a compact ledger (date · description · amount, income in `money-text`).
- **The loan:** monthly amount, a progress bar (14/60), next date.

**Data — fully built (e1).** `GET /api/app/money` → `Money::Page.for`, serializers in
`app/serializers/api/app/money/` (`page_serializer` = cash/needs-you/reconciled/loan,
`needs_you_item_serializer` = Hunt/Confirm/Mark-paid lines, `obligation`/`reconciliation`/
`bank_transaction`/`loan`). Line actions: `POST /api/app/reconciliations/:id/bank_transactions/:id/
{confirm,reject,exclude,reset,manual_match,request_invoice,upload_and_link}`; loans at
`/api/app/money/loans`. ⚠️ Gated by `Features.accounting?` + the `:accounting` entitlement — that
must be ON for the target users, since accounting is a core pillar (flag to ops).

## Calendar

Familiar agenda. Days (Today / named days) with events + the deadlines Scout tracks + kept
focus blocks (dashed). Event block = colour-ticked by place, title + a sub line.

**Data — fully built (e1).** Use `GET /api/app/time` → `Time::Agenda` (serializer
`app/serializers/api/app/time/agenda_serializer.rb`) for the merged agenda — it already merges
events + tracked deadlines/reminders + kept focus blocks + a day note, exactly this surface. Use
`GET /api/app/calendar` → `Calendars::PageData` for pure event views (agenda/day/week/month) + CRUD.

## Skim  (a mode, not a place)

A focus lens launched over any **auto-curated** set — never a deck you assemble.
- Full-surface overlay: a progress bar (one segment per card) + count + close (Esc).
- One large card at a time: Avatar, title, Scout read, optional draft, and **big actions with
  number keys `1–n`**. Acting flies the card out (primary → right, dismiss → left) and advances.
- Ends on a "cleared" state (`happy` Visor). Sources today: **Today's worklist** and the
  **Inbox long tail**; extensible to new documents. Touch: swipe L/R mirrors dismiss/primary.
- Reduced-motion: cards cross-fade, no fly-out.

## Build order

1. Primitives (Visor/Signal/StatusGlyph/Avatar) — landing in `lib/ui` (b4, in flight).
2. Shell (Sidebar + Scout bar + router) + the **Inbox pilot** (list + archive/undo) — first
   running surface, against e1's People endpoint (see `inbox-pilot.md`).
3. Today, then Books, then Calendar. Skim as a shared overlay wired into Today + Inbox.
Each surface: dark + light, 375px + desktop, keyboard path, states (loading skeleton / empty /
error), a Playwright smoke test, Storybook for new `lib/ui` pieces, token lint clean. b4 reviews.
