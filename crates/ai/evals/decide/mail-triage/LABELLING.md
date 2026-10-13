# Labelling guide — `mail-triage`

How every item in this eval set is labelled. A label answers what the
conversations module asks of each inbound message (`docs/domain-map.md` →
*conversations*). When two readers disagree, the ruling goes into this guide
(**Rulings**, at the end) so the next reader agrees.

Everything here is fictional: invented people and organisations, and only
reserved domains (`example.com`, `example.org`, `example.net`, or any name
under `.example`). Genuine senders and impersonations use the same top-level
domain, so the domain's ending says nothing about the label. No real brand, bank, carrier, public
body or person, not even in a phishing look-alike.

## Item format

One JSON object per line in `items/<locale>.jsonl`:

| Field | Meaning |
| --- | --- |
| `id` | `<locale>-<workspace>-<nn>`, unique. |
| `lang` | `en`, `pt-PT`, `es-ES` or `fr`: the language most of the body is in. |
| `split` | `dev` (tune rules, fit thresholds and temperatures) or `test` (report). |
| `workspace` | Whose inbox it is (see `manifest.json`). |
| `hard` | Hard-case tags (below); empty for an ordinary item. |
| `message.from` / `message.to` / `message.cc` | `{name, address}`. |
| `message.subject`, `message.date` | Subject; RFC 3339 date the message was received. |
| `message.headers` | Only the headers that matter to triage and are present (`List-Unsubscribe`, `List-Id`, `Precedence`, `Auto-Submitted`, `Reply-To`, `X-Mailer`, …), as strings. Absent means the provider didn't give it. |
| `message.attachments` | `{name, content_type}` per attachment. |
| `message.body` | Plain text, as the reader would see it (quoted replies and signatures included). |
| `labels` | One value per question (below). |
| `rationale` | One sentence on why, in the item's language. |

## Questions

Everything is judged **from the recipient's point of view, at
`message.date`**.

### `stream` — where the conversation belongs

- **`primary`** — a person (or a small team writing as people) writing to
  the recipient about something between them: clients, colleagues,
  suppliers' staff, family, the school teacher, the landlord. A person sending
  an invoice by hand is `primary`.
- **`updates`** — automated notifications that aren't about money:
  shipping and delivery, account and security alerts, appointment and
  calendar notifications, password resets, app notifications, out-of-office
  auto-replies, document-ready notices.
- **`receipts`** — automated money mail from a system: receipts, order
  confirmations with an amount, invoices and bills issued by a service,
  payment confirmations and failures, refunds, statements.
- **`newsletters`** — content or marketing sent to a list: newsletters,
  promotions, digests, product announcements, event marketing, including a
  newsletter written by one person.

### `needs_reply` (true/false)

The sender expects a **written reply** from the recipient: a question to
them, a request to confirm, an open thread waiting on them. Only `primary`
messages can need a reply. Implies `asks_action`.

### `asks_action` (true/false)

The sender asks the recipient to **do** something: reply, pay, sign, send,
review, book, confirm, renew, verify. A bill from a service that asks to be
paid counts; a receipt for something already paid doesn't. Marketing calls to
action ("shop now", "register today") don't count. Conditional asks ("if this
wasn't you, reset your password") don't count. A suspicious message that
asks you to click, pay or log in counts (it does ask).

### `has_deadline` (true/false)

The message states or clearly implies **a time by which the recipient must
act, or after which something affecting them lapses**: a due date, "by
Friday", "before the end of the month", "today", "within 48 hours", an expiry
of their document or subscription. A meeting time on its own is not a
deadline. Marketing urgency ("sale ends Sunday") is not a deadline. A
suspicious message's deadline is labelled as stated.

### `urgency` (0, 1, 2)

- **0** — nothing for the recipient to do, or no time pressure (FYI,
  newsletters, receipts already paid, notifications).
- **1** — should be handled within days: a question waiting for an answer,
  a deadline more than 24 hours away.
- **2** — today: a deadline within 24 hours, overdue, or explicitly "now /
  today / urgent" with a credible reason.

`urgency > 0` implies `asks_action`; `urgency = 2` implies `has_deadline`.
A suspicious message is always `urgency = 0`: Campbooks never rushes anyone
on a suspicious message.

### `suspicious` (true/false)

Phishing, scams and impersonation: a sender domain that imitates a known one
(`bancoatlantico-seguranca.example` for `bancoatlantico.example`, digits for
letters), links whose text and target disagree, credentials or card details
asked for through a link, gift-card or urgent-transfer requests from a "boss"
or "supplier" at an odd address, fake invoices from unknown vendors with
pressure. A genuine alert from the real sender's domain is **not**
suspicious, however alarming. A suspicious message is labelled with the
`stream` it pretends to be, and never `needs_reply`.

## Hard-case tags

| Tag | What makes it hard |
| --- | --- |
| `polite_ask` | The ask is softened ("would you mind…", "quando puder…", "si tu as un moment…"). |
| `deadline_in_prose` | The deadline is in words, not a date ("before the holidays", "até ao fim da próxima semana"). |
| `receipt_vs_invoice` | Money mail where paid vs to-pay, or system vs person, decides the labels. |
| `invoice_from_person` | A person sends an invoice or quote by hand (`primary`). |
| `newsletter_from_person` | A list mailing written by one person in a personal voice (`newsletters`). |
| `fyi_from_person` | A person writes with nothing to do. |
| `phishing_lookalike` | Suspicious, imitating a known sender. |
| `legit_alert` | A genuine, alarming alert (not suspicious). |
| `conditional_ask` | "If this wasn't you…", "if you have questions…" (not an ask). |
| `marketing_urgency` | Marketing that sounds urgent (no deadline, no ask). |
| `no_bulk_headers` | Automated mail without `List-*`, `Precedence` or `Auto-Submitted`. |
| `quoted_thread` | The new text is short; most of the body is quoted history. |
| `mixed_language` | Body or footer in more than one language. |
| `auto_reply` | An out-of-office or other automatic reply from a person's address. |

## Rulings

- An order confirmation with an amount paid is `receipts`; the shipping
  update for the same order is `updates`.
- A service's reminder that a bill is due ("your invoice is due on 15 May")
  is `receipts`, `asks_action`, `has_deadline`.
- A calendar invitation sent by a person's calendar on their behalf is
  `updates` unless the person added their own words asking something.
- An out-of-office reply is `updates`, nothing to do.
- A password-reset link or sign-in code the recipient just asked for is
  `updates` with no ask and no deadline, even if the link expires: they
  started the action themselves.
- `updates` can ask for an action, carry a deadline and be urgent (an
  appointment to confirm by a date, a freezer alarm that needs someone now).
- A domain or subscription expiry notice with no amount is `updates`; with
  an amount to pay it is `receipts`.
- A genuine alert whose only ask is conditional is no ask, no deadline,
  urgency 0.
- A bill that will be debited automatically is `receipts` with no ask and no
  deadline.
- An action that has to be done before a dated meeting ("bring the samples
  to Thursday's meeting") has a deadline: the meeting is when it is due. A
  question about the meeting itself ("are you both coming?", "does Saturday
  work?", "how many places?") has no deadline unless a reply-by is stated.
- `needs_reply` is true whenever the sender waits for an email from the
  recipient: an answer, a confirmation, or a document sent back by email,
  even if the sender says a bare attachment is enough. It is false when the
  action happens elsewhere (pay, sign on a portal, bring, hand in a form).
- "Now", "right away", "as soon as you get this" count as a deadline of
  today.
- Standard reminder boilerplate ("bring your health card", "have these
  documents at hand") is not an ask; "confirm by …" is.
- A card or fraud alert is `updates` even when it shows an amount; it is not
  money mail about a payment to make or made.
- A fake parcel or customs fee pretends to be a carrier notice: `updates`.
- Someone at an organisation writing to the recipient in their own words (a
  surgery's secretary confirming an appointment the recipient asked for) is
  `primary`.
- A question waiting for an answer is urgency 1 even without a date.

## Agreement (v1)

Every item was drafted to a target label vector, then labelled again blind
from this guide alone. Share of items on which the two passes agreed, before
adjudication:

| Question | en | pt-PT | es-ES | fr | all |
| --- | --- | --- | --- | --- | --- |
| stream | 1.00 | 0.99 | 0.99 | 0.97 | 0.99 |
| needs_reply | 0.94 | 0.96 | 1.00 | 0.99 | 0.97 |
| asks_action | 1.00 | 0.99 | 0.97 | 1.00 | 0.99 |
| has_deadline | 1.00 | 0.97 | 0.91 | 0.99 | 0.97 |
| urgency | 1.00 | 0.99 | 0.97 | 1.00 | 0.99 |
| suspicious | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |

23 of 320 items had a disagreement (29 labels). Each was settled against the
guide and the rulings above were added. Both passes were made by language
models, so this agreement is an upper bound on what two people would reach;
the hardest calls (what counts as a reply, deadlines tied to events) are
where the disagreements were.
