# Design

## Context

Two pieces meet here:

- **Taught rules** (#125, change `teach-once`): deterministic rules in
  `conversations.rules`, one per inbox, evaluated first in the decide
  cascade (taught > signals > k-NN > model), each `conditions (AND) →
  actions`, with a preview operation and an origin. This change compiles
  into that model and defines none of its own.
- **Onboarding** (#73) and **Scout's memory**: where the questions are
  asked and where what Scout was taught is shown afterwards.

The owner's rules for this work: ask, don't make people write; one
question per view, everything skippable, never forced; what a person
teaches becomes deterministic rules; AI compiles a sentence into rules,
the person confirms with a preview, and the saved rule runs without AI.
Nothing the person teaches goes into a decision model's prompt ("for now
we leave that be"): a sentence that cannot become a rule gets an honest
answer, never a hidden note. Copy never promises an effect that does not
exist.

## Goals

- Few questions, each of which changes something deterministic.
- Everything taught visible afterwards as a rule sentence, editable and
  removable, in four languages.
- The compile model is called only on the person's own action, and never
  receives anyone's mail.
- The same mail always gets the same result once a rule exists.

## Decisions

### 1. The questions, and what each one changes

| # | Question (en) | What it changes |
| - | --- | --- |
| 1 | Whose mail should always reach you? Suggestions (who you reply to most) plus "Add an address or domain" | One taught rule per sender: `sender_address is X` (or `sender_domain is D`, never a free-mail domain) → `surface: always`. Their mail always raises at least a Review need and no rule hides it. |
| 2 | What never needs you? Newsletters and mailing lists / Automatic notifications / senders you rarely open | Taught rules → `surface: never`: `header list_id present`; `header auto_submitted present`; `sender_address is X` per sender. No need is raised for them. Suspicious mail is exempt (#125). |
| 3 | When Scout is sure, should it act? Ask me first / Do it and tell me | `proactivity` on the member's profile, read by #125 through the kernel port `MemberPreferences::acts_without_asking`: a rule Scout proposes from that member's evidence starts `active` with a notice and Undo in a personal inbox, and is always `proposed` in a shared inbox. Default `ask_first`, #125's behaviour without the port. |

Question 3 is shown only once #125's port is on `next` (a client flag
until then), so the flow never asks a question with no effect.

Dropped, with the reason:

- **What you use Campbooks for / what you do**, and **what matters most**:
  nothing deterministic reads them. Their only reader was a note for the
  decision model, which the owner ruled out for now. A person who wants
  "invoices matter" says so in Teach Scout, which turns it into a
  previewed rule ("Always show mail with 'invoice' in the subject").
- **Quiet or thorough**, tone, business name: nothing reads them yet.

### 2. What the person sees for each answer

Questions 1 and 2 show, under each selection, the rule it becomes in the
sentence Scout's memory uses, with the preview from
`conversations.preview_rule` (count and samples). A review view lists every
rule grouped by inbox, with one choice: also apply to the mail you already
have (default on). The member's own inboxes are on. An inbox shared with
others is marked "For everyone" and is off until the member switches it on.
Inboxes the member only reads get no rule, and the view says so. Save
creates every rule (`create_rule`, `state = active`, origin `onboarding`)
and writes the profile; one Undo removes them all.

Until #125's operations are on `next`, the views show the rule sentence
without counts, and Save stores the profile only; wiring the rules is a
task of this change.

### 3. Where answers live

**The profile: per member, per workspace**, in `assistant.profiles`:

```
profiles (workspace_id, user_id) PK
  proactivity   text         -- ask_first | act_then_tell (default ask_first)
  asked_at      timestamptz null  -- when the questions were answered or put off
  revision      int          -- +1 on every change
  updated_at    timestamptz
```

- Forced RLS on `workspace_id`, like every module schema, and the rows are
  keyed by the member: every query filters on the caller's own user id.
- ADR 0017 resource type `assistant.profile`: tenancy workspace, owner the
  member, owner actions `read` and `write`; no grant to any workspace role
  (owners and admins cannot read a member's profile), none to client
  tokens. Matrix tests show an owner and an admin get nothing of another
  member's.
- Purged on `workspaces.member_left` and `workspaces.workspace_deleting`.

**The rules: per inbox**, in #125's `conversations.rules`, created by the
member (`sender` role, which `preview_rule` and `create_rule` require).

### 4. Compiling a sentence into rules

`POST /api/v1/w/{ws}/assistant/compile` (`assistant.compile_instruction`,
scope `assistant:write`, not mutating, rate-limited to 20 per member per
hour). Called by the client only when the person presses Enter or Teach.

Input: `{ text }`, at most 300 characters. Steps:

1. **Sanitise** the text: displayable characters only
   (`kernel::text::displayable`: no control, bidi or zero-width
   characters), whitespace collapsed, trimmed. Refuse with 400
   `instruction_invalid` and a `reason` (`empty`, `too_long`, `url`,
   `pasted_mail`) when it is empty, longer than 300 characters, holds a URL,
   or looks pasted from mail (header lines such as `From:` or `Subject:`,
   quote markers, more than two addresses).
2. **Gather the person's own context** through `kernel::operations::invoke`
   as the person (`Caller::Assistant`), so every access check applies:
   their inboxes' addresses (`conversations.list_inboxes`) and their label
   names (`conversations.list_labels`). Stream keys are fixed. No sender,
   subject or body.
3. **Ask the `complete` role** through `Gateway::chat`, which uses only a
   provider that may receive content (an external one needs
   `CAMPBOOKS_AI__COMPLETE__EXTERNAL_CONTENT=allow`). Temperature 0, a strict
   JSON-schema response format, no tools. The system message is a fixed
   English instruction; the user message is one JSON object
   `{ "sentence", "my_addresses", "streams", "labels" }`, so the sentence is
   a JSON string, never instructions. The output:

   ```
   { "outcome": "rules" | "unsupported" | "unclear",
     "unsupported": "none" | "sends_mail" | "deletes_mail" | "not_about_mail" | "too_vague",
     "rules": [ { "who": "",                         // a phrase naming someone, or ""
                  "conditions": [ { "field", "value" } ],
                  "actions": [ { "kind", "value" } ] } ] }
   ```
4. **Validate on the server** against the closed vocabulary and the inputs.
   Fields: `sender_address`, `sender_domain`, `subject_contains`,
   `header_present` (`list_id`, `list_unsubscribe`, `auto_submitted`,
   `precedence`), `label`, `stream`. Actions: `move_to_stream`, `archive`,
   `add_label`, `mark_read`, `needs_reply`, `asks_action`, `surface`.
   - An address or domain must appear in the sentence or be one of the
     person's own inbox addresses (or their domain); a domain is never a
     free-mail domain.
   - Every `subject_contains` word must appear in the sentence.
   - A label must be one of the person's labels; a stream one of the four.
   - One action per slot, 1 to 4 conditions (or a `who`), 1 to 4 actions,
     at most 3 rules.
   - A `who` phrase must appear in the sentence.

   A rule that fails any check is dropped; if nothing valid is left, the
   outcome is `unclear`. Nothing is repaired by guessing.
5. **Resolve `who`** deterministically: an address or domain in the
   phrase is used as is; otherwise `conversations.list_senders` (as the
   person) finds senders whose name, address or domain contains the
   phrase's words, ranked by how often the person replied. The answer
   carries them as candidates; the client asks "Who is your accountant?"
   and the person picks one or types an address. Scout never picks.

Output: proposals in the shape `conversations.create_rule` takes
(`conditions`, `actions`), each with an optional `who` and its candidates,
plus the outcome and the unsupported reason. The client previews each
proposal with `conversations.preview_rule` and saves with
`conversations.create_rule` (`state = active`, origin `instruction`,
`origin_text` = the sanitised sentence). **The model never saves anything.**

An `unsupported` or `unclear` outcome gets an honest answer in the
person's language ("Scout can't send or forward mail.", "Scout couldn't
turn that into a rule. Try naming a sender, a word in the subject, or what
to do with it.") plus the closest valid rule, if one is left. Nothing is
stored.

Without a usable `complete` role (`RoleNotConfigured` or
`ExternalContentRefused`), the operation answers 503
`ai_role_not_configured` / `ai_external_content_refused` (the gateway's
reasons) without calling anything, and the client shows the manual
builder: "When [the sender is ▾] [ … ] then [always show it ▾]", with the
same preview and Save. The questions need no model at all.

### 5. Scout's memory

The page is at `/w/{id}/settings/memory`. #125 owns the rules list on it;
this change adds the Teach Scout composer, the manual builder and "Answer
the questions again". Rules with origin `onboarding` read "From your
answers"; rules with origin `instruction` quote the person's words.

### 6. Security

- **What reaches a model**: compile only, on the person's action: their
  sentence, their own inbox addresses, the stream keys and their label
  names. Never another person's address, name or mail text; never in the
  background. Candidates and previews are computed in Postgres and shown
  only to the person.
- **Nothing taught reaches the decision model.** Rules decide; the
  decide pass's prompt is unchanged by this change.
- **The compile model has no power**: no tools, strict schema, closed
  vocabulary, validated on the server against the sentence and the
  person's own data. Nothing sends, forwards, replies or deletes. Every
  rule is shown with its preview and saved by the person; suspicious mail
  is exempt from hiding rules (#125).
- **Errors**: the gateway never keeps a provider's error body; compile
  logs only the error kind.
- **Access**: profiles are member-private (resource type, RLS, own-id
  filter); compile runs the conversations operations as the person, so
  their checks apply and an inbox the person can't see doesn't exist.
- **Telemetry**: no text, no addresses; compile records only its outcome,
  latency and counts.
- **Cost**: 300 characters, 20 compiles per member per hour.

### 7. Defaults when skipped

| Question | Skipped means |
| --- | --- |
| 1 | No rules. Home behaves as today. |
| 2 | No rules. Built-in signals keep sorting as today. |
| 3 | `ask_first`. |
| The whole flow ("Not now") | `asked_at` set; Home stops offering it; Scout's memory always has "Answer the questions". |

### 8. Languages

All copy in en, pt-PT, es-ES and fr, with full diacritics, in the client
module's `i18n/` catalogues; the server's reasons in the module's
catalogue.

| key | en | pt-PT | es-ES | fr |
| --- | --- | --- | --- | --- |
| intro | Tell Scout what you care about | Diga ao Scout o que lhe importa | Dile a Scout lo que te importa | Dites à Scout ce qui compte pour vous |
| q1 | Whose mail should always reach you? | De quem quer ver sempre o correio? | ¿De quién quieres ver siempre el correo? | De qui voulez-vous toujours voir les messages ? |
| q2 | What never needs you? | O que nunca precisa de si? | ¿Qué nunca te necesita? | Qu’est-ce qui n’a jamais besoin de vous ? |
| q3 | When Scout is sure, should it act? | Quando o Scout tem a certeza, deve agir? | Cuando Scout está seguro, ¿debe actuar? | Quand Scout est sûr, doit-il agir ? |
| q3 options | Ask me first · Do it and tell me | Pergunte-me primeiro · Faça e avise-me | Pregúntame antes · Hazlo y avísame | Demandez-moi d’abord · Faites-le et prévenez-moi |
| keys | Next · Skip · Not now | Seguinte · Saltar · Agora não | Siguiente · Omitir · Ahora no | Suivant · Passer · Pas maintenant |
| footer | You can change this anytime in Scout’s memory. | Pode alterar isto a qualquer momento na memória do Scout. | Puedes cambiarlo cuando quieras en la memoria de Scout. | Vous pouvez changer cela à tout moment dans la mémoire de Scout. |

### 9. Keyboard

Registered in `lib/shortcuts` with namespaced labels and shown with the
registry's hints: `Enter` next or save, `S` skip this question, `Esc` not
now, `1`–`9` toggle the visible options. In the composer, `Enter` teaches,
then `Enter` saves the shown rule, `Esc` cancels.

## Data ownership, events, scopes

- Schema `assistant`: `profiles`. Compile results are not stored.
- Scopes: `assistant:read` (`get_profile`), `assistant:write`
  (`update_profile`, `compile_instruction`).
- Operations (OpenAPI, ADR 0013): `assistant.get_profile` (GET),
  `assistant.update_profile` (PUT, its own undo),
  `assistant.compile_instruction` (POST, not mutating). In
  `conversations`: `conversations.list_senders` and
  `conversations.list_labels` (GET, `conversations:read`, callable by the
  assistant), and `InboxView.shared`.
- Events: none published. Consumed: `workspaces.member_left`,
  `workspaces.workspace_deleting` (purge).
- Port: `assistant` implements #125's
  `kernel::preferences::MemberPreferences::acts_without_asking`.
- AI: role `complete` for compile only; fallback the manual builder; undo
  is removing the rule (`remove_rule`, with `restore`).

## Alternatives considered

### Which questions

- **A free-text "instructions for Scout" box** (#73's advanced field)
  sent to the model with every decision. Cheapest, but it makes people
  write and gives a model unbounded personal text that decides nothing
  deterministically. Ruled out by the owner; Teach Scout compiles a
  sentence into rules instead.
- **The owner's full list** (who you are, what matters most, quiet or
  thorough). Nothing deterministic reads those answers today, so asking
  them would promise effects that do not exist.
- **Chosen: three questions**, each tied to rules or to how Scout's own
  rule proposals start.

### Where the profile is stored

- **Per workspace** (one shared profile): a member's choices would become
  everyone's, and admins would edit them.
- **Per inbox**: a person with two mailboxes would answer twice; rules
  stay per inbox as #125 decides.
- **Chosen: per member per workspace**, member-private.

### Which module owns it

- **`workspaces`** (it stores a member's locale): no new module, but
  tenancy would gain AI and rule compilation.
- **`conversations`** (where the rules are): no invoke hop, but the
  profile and compilation are not about messages, and later rule kinds
  (documents, money) would go through conversations.
- **Chosen: a new `assistant` module**, as the domain map assigns Memory
  to it (owner decision 4).

### How language becomes rules

- **Hand-written parsers per language**: deterministic and free, but
  brittle across four languages; the owner chose AI (#127).
- **The model writes rules into storage**: the model would act, which
  the AI policy forbids.
- **Chosen: the model proposes in a closed schema; the server validates
  and resolves names; the person previews and saves.**

### How the person's context reaches compile

- **The client sends its inboxes and labels**: no invoke, but the server
  would validate against data the caller chose.
- **Chosen: invoke the conversations operations as the person**, so the
  context is the person's real data and every access check applies.

### Copying from the Rails app on `main`

The Rails app had a "Scout's memory" page derived from the real behaviour
records and a "Teach Scout" box parsed by hand-written parsers. This
change keeps one idea because it wins on the merits: **memory is a view
over the real rules, never a separate store**. It does not copy the
parsers or the free-form AI prompt settings.

## Risks

- **#125 lands underneath.** Compile output is its request shape; wiring
  previews and saves is a task here, done when its operations are on
  `next`.
- **`complete` quality on short sentences in pt/es/fr.** The person always
  sees the rule and its preview before saving; an eval set follows.
- **A rule that hides too much.** Free-mail domains are refused; previews
  show the count; suspicious mail is exempt; Undo restores.

## Decisions taken

1. Onboarding answers may create rules in a shared inbox, off by default
   and marked "For everyone" in the review view.
2. The default proactivity is "ask first".
3. The questions come after mail is connected (offered on Home).
4. Answers live in the new `assistant` module, member-private.
