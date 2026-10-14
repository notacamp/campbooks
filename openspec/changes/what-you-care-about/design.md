# Design

## Context

Three pieces meet here:

- **Taught rules** (#125, change `teach-once`): deterministic rules in
  `conversations.rules`, one per inbox, evaluated first in the decide
  cascade (taught > signals > k-NN > model), each `condition (AND) →
  actions`, with a preview operation and an origin. This change compiles
  into that model and defines none of its own.
- **The decision context** (Laya fix, branch `fix/laya-decide-mapping`): a
  versioned block every `decide` request carries, with a slot for "how this
  person wants the AI to act". This change fills that slot.
- **Onboarding** (#73) and **Scout's memory**: where the questions are
  asked and where the answers live afterwards.

The owner's rules for this work: ask, don't make people write; one
question per view, everything skippable, never forced; precise answers
become deterministic rules; the model's preferences are a fallback; AI
compiles a sentence into rules, the person confirms with a preview, and
the saved rule runs without AI.

## Goals

- Five questions or fewer, each of which changes something measurable.
- Every answer visible afterwards as a plain sentence, editable and
  removable, in four languages.
- Nothing typed by the person reaches a model without the person's own
  action, and nothing from anyone's mail reaches the model through this
  feature at all.
- The same mail always gets the same result once a rule exists.

## Decisions

### 1. The questions, and what each one changes

| # | Question (en) | Kind | What it changes |
| - | --- | --- | --- |
| 1 | What do you use Campbooks for? Work / Personal life / Both, plus "In a few words, what do you do?" (optional, 80 chars) | single choice + short text | Preferences block (`uses`, `does`). Orders the topic chips in question 2 (work first or home first). Nothing deterministic: a role is not a rule. |
| 2 | What matters most to you? Up to three of: Clients, Invoices and payments, Suppliers, Appointments, Team, Taxes, Family, School | multi choice | Preferences block (`matters_most`). The model's fallback weighs these for mail no rule covers. This is the one question with no deterministic effect, kept because topics cannot be matched on headers and the owner asked for it. |
| 3 | Whose mail should always reach you? Suggestions plus "Add a name, address or domain" | senders | One taught rule per sender: `sender_address is X` (or `sender_domain is D`, never a free-mail domain) → `surface: always`. Their mail always raises at least a Review need and no rule hides it. |
| 4 | What never needs you? Newsletters and mailing lists / Automatic notifications / suggested senders | chips + senders | Taught rules → `surface: never`: `header list_id present`; `header auto_submitted present`; `sender_address is X` per sender. No need is raised for them. Suspicious mail is exempt (#125). |
| 5 | When Scout is sure, should it act? Ask me first / Do it and tell me | single choice | Profile `proactivity`. Deterministic effect: rules Scout proposes from evidence (#125 origins `suggestion` and `correction`) are created `proposed` (ask) or `active` with a notice and Undo (act). Also a sentence in the block. It never widens the eval policy (ADR 0009 note). Needs #125 to read it through the port when it creates those rules (today #125 always creates them proposed, which is the `ask_first` behaviour). |

Dropped from the owner's list, with the reason:

- **"Quiet or thorough"**: nothing in the product reads it yet (Home has
  one density, notifications don't exist). Asking a question that changes
  nothing breaks the rule above. Revisit with notifications.
- **Business name**: the workspace already has a name (first-run asks it).
  The block uses the workspace name for `uses: work`.
- **Languages and tone for drafting** (#73 step 2): belongs to drafting
  (`complete` for replies), which does not exist yet.

Order follows the cost of answering: two taps, then picks from
suggestions, then the one judgement call.

### 2. What the person sees for each answer

Questions 3 and 4 show, under each selection, the rule it becomes, in the
sentence Scout's memory will use, with the preview from
`conversations.preview_rule`: "Always show mail from Rita Gomes
(rita@silva-contas.example) · 41 emails, newest: Q3 VAT figures". Before
anything is saved, a review view lists every rule and every "keep in mind"
sentence, with one choice: apply to the mail you already have (default on,
with the total count) or only to new mail. Save creates every rule in one
step (`create_rule`, `state = active`, origin `onboarding`) and writes the
profile. Undo on the toast removes them all (`remove_rule` per rule,
`assistant.update_profile` back to the previous revision).

### 3. Where answers live

**The profile: per member, per workspace**, in `assistant.profiles`:

```
profiles (workspace_id, member_id) PK
  uses          text null    -- work | personal | both
  does          text null    -- the member's words, ≤ 80 chars, sanitised
  matters_most  text[]       -- closed set of topic keys, ≤ 3
  proactivity   text         -- ask_first | act_then_tell (default ask_first)
  notes         jsonb        -- ≤ 3 × { id, text ≤ 200 chars, created_at }
  asked_at      timestamptz null  -- when the flow was last completed or skipped
  revision      int          -- +1 on every change
  updated_at
```

- Forced RLS on `workspace_id` like every module schema.
- ADR 0017 resource type `assistant.profile`: tenancy workspace, owner the
  member, owner actions `read`, `write`, `delete`; **no grant to any
  workspace role** (owners and admins cannot read a member's profile, as
  with an inbox they are not in); client tokens: `read` only with
  `assistant:read`. Every handler calls `authz::authorize` on the
  caller's own profile; there is no way to name another member's.
- Purged on `workspaces.member_left` and `workspaces.workspace_deleting`.

**The rules: per inbox**, in #125's `conversations.rules`. An answer
creates one rule per inbox the member may teach in (`sender`+, which
`preview_rule` and `create_rule` require), with one `teaching_id`, shown as
one sentence. A rule acts for every member of its inbox (#125). So the
review view groups rules by inbox: the member's own inboxes are on; an
inbox shared with others is marked "For everyone" and is off until the
member switches it on (owner question 1). Inboxes where the member is only
a `reader` get no rule, and the view says so.

**Whose preferences a decision reads**: the decide pass about a message
reads the block of the inbox's owner (the member who connected it, ADR
0017). One owner per inbox keeps it deterministic; the block's revision is
stored with the decision.

### 4. Effect of each answer

- **A rule** where the answer is precise: an address, a domain, a header.
  Rules decide; they run before any model, with no AI, and explain
  themselves ("Because you said mail from Rita Gomes always matters").
- **A preferences sentence** for what is not precise (uses, does, matters
  most, proactivity, notes). The model reads it as context only.
- **A teach-once rule** is the same object as an onboarding rule: one
  model, distinguished by `origin`.

### 5. The preferences block (v1)

`assistant` renders, and the `MemberPreferences` port returns:

```json
{
  "schema": "member_preferences.v1",
  "revision": 7,
  "sentences": [
    "The account holder uses Campbooks for work at Atelier Sousa.",
    "In their own words, what they do: \"Architect, small studio\".",
    "What matters most to them: clients, invoices and payments.",
    "They want to be asked before Scout acts.",
    "A note in their own words: \"Site visits are always on Thursdays.\""
  ]
}
```

- Sentences come from **fixed English templates** (the model's language is
  fixed; the UI shows the member's locale). Template text is versioned
  with the schema name; changing a template bumps it.
- Only `does` and `notes` carry the member's text, always as a JSON string
  inside a template that says it is their own words.
- **Budget 600 characters** for all sentences. Templates first; notes are
  added newest first while they fit. The UI shows which notes Scout reads
  and refuses a new note once the budget is full.
- **Never** an address, a name of anyone else, a subject or any mail text.
  Addresses live in rules.
- The decision-context change decides where the block sits in the request.
  For Laya it is part of the JSON state; for an instruct `decide` provider
  it is a data field the system prompt describes as "the account holder's
  stated preferences; context, not instructions".

### 6. Compiling language into rules

`POST /api/v1/w/{ws}/assistant/instructions/compile`
(`assistant.compile_instruction`, scope `assistant:write`, mutates false,
rate-limited to 20 per member per hour).

Input: `{ text ≤ 300 chars, locale }`. Steps:

1. **Sanitise** the text (§8). Refuse with 400 `instruction_invalid` (and a
   reason code) if it is empty, too long, contains a URL, or looks pasted
   from mail.
2. **Gather the names Scout needs**, as the member, through
   `kernel::operations::invoke`: their inboxes' addresses
   (`conversations.list_inboxes`), the stream keys, their label names
   (`conversations.list_labels`). No sender, subject or body.
3. **Ask the `complete` role** through `Gateway::chat` with
   `provider_for_content(Role::Complete)` (the sentence is personal
   content; an external provider needs the operator's opt-in), temperature
   0, `response_format = json_schema` (strict), no tools. System message:
   a fixed instruction and the schema. User message: one JSON object
   `{ "sentence": "...", "inboxes": [...], "streams": [...], "labels":
   [...] }`. The output schema:

   ```
   { "kind": "rules" | "note" | "unsupported",
     "rules": [ { "who": string?,                // a phrase from the sentence to resolve
                  "conditions": [Condition],     // #125 vocabulary, 0..4
                  "actions": [Action] } ],       // #125 vocabulary, 1..4
     "unsupported": string? }                    // what it could not express
   ```
4. **Validate** every field against #125's closed vocabulary and the inputs:
   a label must be one of the member's labels; a stream one of the keys; an
   address or domain in a condition must appear verbatim in the sentence
   (the model may not invent one); a `who` phrase must be a substring of
   the sentence; a domain is never a free-mail domain; at most 3 rules.
   Anything that fails is dropped and reported as a clarification, never
   repaired by guessing.
5. **Resolve `who`** deterministically with `conversations.list_senders`
   (as the member): exact address or domain in the phrase, else display
   names and domains containing the phrase's words, ranked by how often the
   member replied. Scout then asks: "Who is your accountant?" with the top
   candidates and a type-ahead. It never picks for the person.
6. **Preview** each resolved rule with `conversations.preview_rule`
   (computed on the server, deterministic, no model).

Output: proposals in `conversations.create_rule`'s request shape (origin
`instruction`, `origin_text` = the sanitised sentence) with their
previews, plus clarifications. `kind: note` returns the sentence as a
proposed note for the profile; `kind: unsupported` explains what Scout
can't do ("Scout can't forward mail. It can always show these to you.")
and offers the closest rule if one exists.

The client saves with `conversations.create_rule` (and `confirm_rule` if
created proposed); a note with `assistant.add_note`. **The model never
saves anything.**

Without a usable `complete` role (`RoleNotConfigured` or
`ExternalContentRefused`), the operation answers 409 `compile_unavailable`
and the client shows the manual builder: "When [sender is ▾] [ … ] then
[always show ▾]", the same preview and Save. The question flow does not
need `complete` at all: its answers map to rules with fixed code.

### 7. Scout's memory

The page (`/w/{id}/settings/memory`) and its rules list belong to #125's
`memory-ui` capability. This change adds to the same page:

- **About you**: one sentence per profile answer and note, in the
  member's locale ("You use Campbooks for work at Atelier Sousa", "Clients
  and invoices matter most to you", "Ask before acting"), each with Change
  and Remove; "Answer the questions again" reopens the flow with the
  current answers selected; "What Scout reads" shows the exact English
  block.
- **Your rules** (#125): this change only supplies the origin lines for
  its producers ("From your answers", "You wrote: 'anything from my
  accountant always matters'").
- **Teach Scout something**: the composer of §6, at the top of the page.

### 8. Security

- **What reaches a model.** Compile: the person's sentence, their own
  inbox addresses, stream keys, their label names. The decide pass: the
  preferences block. Never another person's address, name or mail text
  through this feature; previews and suggestions are computed in Postgres
  and shown only to the member.
- **Sanitising the person's text** (`does`, notes, instructions): Unicode
  NFC; remove control and format characters (Cc, Cf: bidi overrides,
  zero-width); collapse whitespace and newlines to one space; trim; length
  caps (80, 200, 300); refuse URLs and text that looks like pasted mail
  (header lines, quote markers, more than one line before collapsing, a
  run of addresses). Stored only after sanitising, so what is shown is what
  is used.
- **Rendering into model input as data**: always inside a JSON string
  (escaping makes delimiter breakout impossible), labelled as the account
  holder's own words, inside a fixed instruction the person cannot change.
- **The compile model has no power**: no tools, strict schema, closed
  vocabulary, validated against the inputs, at most 3 rules; actions are
  only `move_to_stream`, `archive`, `add_label`, `mark_read`,
  `needs_reply`, `asks_action`, `surface`. Nothing sends, forwards,
  replies or deletes. Every rule is shown with its preview and confirmed by
  the person; suspicious mail is exempt from hiding rules (#125).
- **A prompt-injected preference can only bias the fallback**: rules
  decide first, and model answers act only where the eval policy allows.
- **Access**: profiles are owner-only (no admin read), RLS-forced; compile
  and suggestions run as the member, so the ADR 0017 checks of the
  operations they invoke apply; an inbox the member can't see is invisible.
- **Telemetry**: no text, no addresses; compile records only outcome kind,
  latency and the count of rules/clarifications.
- **Abuse and cost**: compile only on the person's Save/Enter (the AI
  policy's "own action"), 20 per hour per member, 300 characters.

### 9. Defaults when skipped

| Question | Skipped means |
| --- | --- |
| 1 | `uses` from the workspace kind (Business → work, Personal → personal); no `does`. |
| 2 | No topics; no sentence. |
| 3 | No rules. Home behaves as today. |
| 4 | No rules. Built-in signals keep sorting as today. |
| 5 | `ask_first`. |
| Whole flow ("Later") | `asked_at` set so the flow does not reopen by itself; Home shows one dismissible "Tell Scout what you care about" offer for 7 days; Scout's memory always has the entry. |

### 10. Languages

All copy in en, pt-PT, es-ES and fr, with full diacritics, in the module's
`i18n/` catalogues. Questions and options (the UI uses "Scout"):

| key | en | pt-PT | es-ES | fr |
| --- | --- | --- | --- | --- |
| intro | Tell Scout what you care about | Diga ao Scout o que lhe importa | Dile a Scout lo que te importa | Dites à Scout ce qui compte pour vous |
| intro lead | Five quick questions, all optional. You can change this anytime. | Cinco perguntas rápidas, todas opcionais. Pode alterar isto a qualquer momento. | Cinco preguntas rápidas, todas opcionales. Puedes cambiarlo cuando quieras. | Cinq questions rapides, toutes facultatives. Vous pouvez changer cela à tout moment. |
| q1 | What do you use Campbooks for? | Para que usa o Campbooks? | ¿Para qué usas Campbooks? | Pour quoi utilisez-vous Campbooks ? |
| q1 options | Work · Personal life · Both | Trabalho · Vida pessoal · Ambos | Trabajo · Vida personal · Ambos | Travail · Vie personnelle · Les deux |
| q1 field | In a few words, what do you do? | Em poucas palavras, o que faz? | En pocas palabras, ¿a qué te dedicas? | En quelques mots, que faites-vous ? |
| q2 | What matters most to you? | O que mais lhe importa? | ¿Qué es lo que más te importa? | Qu’est-ce qui compte le plus pour vous ? |
| q2 options | Clients · Invoices and payments · Suppliers · Appointments · Team · Taxes · Family · School | Clientes · Faturas e pagamentos · Fornecedores · Marcações · Equipa · Impostos · Família · Escola | Clientes · Facturas y pagos · Proveedores · Citas · Equipo · Impuestos · Familia · Colegio | Clients · Factures et paiements · Fournisseurs · Rendez-vous · Équipe · Impôts · Famille · École |
| q3 | Whose mail should always reach you? | De quem quer ver sempre o correio? | ¿De quién quieres ver siempre el correo? | De qui voulez-vous toujours voir les messages ? |
| q4 | What never needs you? | O que nunca precisa de si? | ¿Qué nunca te necesita? | Qu’est-ce qui n’a jamais besoin de vous ? |
| q4 options | Newsletters and mailing lists · Automatic notifications | Newsletters e listas de correio · Notificações automáticas | Boletines y listas de correo · Notificaciones automáticas | Newsletters et listes de diffusion · Notifications automatiques |
| q5 | When Scout is sure, should it act? | Quando o Scout tem a certeza, deve agir? | Cuando Scout está seguro, ¿debe actuar? | Quand Scout est sûr, doit-il agir ? |
| q5 options | Ask me first · Do it and tell me | Pergunte-me primeiro · Faça e avise-me | Pregúntame antes · Hazlo y avísame | Demandez-moi d’abord · Faites-le et prévenez-moi |
| keys | Next · Skip · Later | Seguinte · Saltar · Mais tarde | Siguiente · Omitir · Más tarde | Suivant · Passer · Plus tard |
| footer | You can change this anytime in Scout’s memory. | Pode alterar isto a qualquer momento na memória do Scout. | Puedes cambiarlo cuando quieras en la memoria de Scout. | Vous pouvez changer cela à tout moment dans la mémoire de Scout. |

Rule sentences are rendered client-side from #125's vocabulary with the
same catalogues; the model-facing block is English only.

### 11. Keyboard

Registered in `lib/shortcuts` with namespaced labels and shown with
`KbdHint`: `Enter` next / save, `S` skip this question, `Esc` later (leave
the flow), `Backspace` (outside a field) back, `1`–`8` toggle the visible
options. In the composer, `Enter` compiles, then `Enter` saves the shown
rule, `Esc` cancels.

## Data ownership, events, scopes

- Schema `assistant`: `profiles`. Nothing else stored; compile results
  are not stored (rules live in `conversations`, notes in the profile).
- Scopes: `assistant:read` (`get_profile`), `assistant:write`
  (`update_profile`, `add_note`, `remove_note`, `reset_profile`,
  `compile_instruction`).
- Operations (OpenAPI, ADR 0013): `assistant.get_profile` (GET),
  `assistant.update_profile` (PUT, undo: itself with the previous body),
  `assistant.add_note` (POST, undo `assistant.remove_note`),
  `assistant.remove_note` (DELETE, undo `assistant.add_note`),
  `assistant.reset_profile` (DELETE), `assistant.compile_instruction`
  (POST, not mutating, `assistant`-callable later). In `conversations`:
  `conversations.list_senders` (GET, `conversations:read`).
- Events: none published. Profile changes reach the member's devices on
  the change feed with the member as the only audience. Consumed:
  `workspaces.member_left`, `workspaces.workspace_deleting` (purge).
- Port: `kernel::MemberPreferences::get(tx, workspace, member) ->
  MemberPreferences { block: Option<PreferencesBlock>, acts_without_asking:
  bool }` where `PreferencesBlock { schema, revision, sentences:
  Vec<String> }` is opaque to the kernel. Implemented by `assistant`; read
  by `conversations.decide_message` (the block) and by #125's rule
  suggestions (`acts_without_asking`).
- AI: role `complete` for compile only; fallback the manual builder (409
  `compile_unavailable`); undo is removing the rule (`remove_rule`, with
  `restore` for the mail it touched). Role `decide` reads the block; with
  no `decide` configured nothing changes.

## Alternatives considered

### Which questions

- **A free-text "instructions for Scout" box first** (the classic prompt
  box, #73's "advanced free-text field"). Cheapest to build, but it makes
  people write, gives the model unbounded personal text, and produces
  nothing deterministic. Kept only as "Teach Scout something", compiled
  into rules.
- **The owner's full list including tone, quiet/thorough and business
  details.** More signal for later features, but several answers would
  change nothing today and lengthen the flow. Dropped until something
  reads them.
- **Chosen: five questions, each tied to a rule, the proactivity setting,
  or a short preference sentence.**

### Where the profile is stored

- **Per workspace** (one shared "about this business"). Good for a
  business description, but a member's personal preferences would become
  everyone's, and admins would edit them. Not chosen; a workspace-level
  context can be added later alongside.
- **Per inbox.** Matches where rules live, but a person with two
  mailboxes would answer twice, and "what matters to me" is about the
  person. Not chosen for the profile; rules stay per inbox as #125 decides.
- **Chosen: per member per workspace**, owner-only, with rules per inbox.

### Which module owns it

- **`workspaces`** (it already stores a member's locale preference). No new
  module, but `workspaces` would gain AI and rule compilation, which do not
  belong to tenancy.
- **`conversations`** (where the rules and the decide pass are). No port
  and no invoke hop, but the profile is not about messages, and the next
  decide passes (documents, tasks) would have to read conversations.
- **Chosen: a new `assistant` module**, as the domain map already assigns
  "Memory" to it. Cost: one crate, one schema, one kernel port. Owner
  question 4.

### How the decide pass gets the block

- **Event projection** (`assistant.profile_changed` carrying the block,
  copied into `conversations`). No synchronous call, but the member's own
  words would be copied into a second schema and every purge would have to
  find them.
- **Chosen: a read port** (`MemberPreferences`), like `Visibility`; one
  copy of the text, read at decision time, revision recorded.

### How language becomes rules

- **Hand-written parsers per language** (keywords for "always", "never",
  addresses). Deterministic and free, but brittle across four languages
  and phrasing; the owner chose AI (#127).
- **The model writes rules directly into storage.** Fewest steps, but the
  model would be acting, which the AI policy forbids.
- **Chosen: the model proposes in a closed schema; the server validates,
  resolves names deterministically and previews; the person confirms.**

### Copying from the Rails app on `main`

The Rails app had a "Scout's memory" page that derived sentences from
existing behaviour records (`Scout::Memory::Catalog`) and a "Teach Scout"
box parsed by hand-written parsers. This change keeps one idea because it
wins on the merits: **memory is a view over the real rules and settings,
never a separate store**, so what is shown is what runs. It does not copy
the parsers (replaced by the AI compile step with validation), the
per-behaviour adapters, or the free-form AI prompt settings.

## Risks

- **Two in-flight changes underneath** (#125 and the decision context).
  Mitigation: compile output is #125's request shape, the block is opaque
  to the envelope, and this change merges only after both.
- **`complete` quality on short sentences in pt/es/fr.** Mitigation: an
  eval set of instructions per locale (tasks), and the person always sees
  the rule before saving.
- **A rule that hides too much** ("never needs me: gmail.com"). Free-mail
  domains are refused for domain rules; previews show the count first;
  suspicious mail is exempt; Undo restores.

## Owner questions

1. **Shared inboxes.** A rule acts for everyone in its inbox (#125). When a
   member answers the questions, should their answers make rules in an
   inbox they share with others? *Recommended:* yes, but off by default:
   the review view marks shared inboxes "For everyone" and the member
   switches each one on. Their own inboxes are on. Readers get none.
2. **Default proactivity.** Ask me first, or Do it and tell me?
   *Recommended:* Ask me first, until the eval set shows suggestions hold.
3. **Where in onboarding.** After the mailbox step (previews show real
   counts) or before it (as #73 lists)? *Recommended:* after; without a
   mailbox the flow still runs and says rules apply as mail arrives.
4. **A new `assistant` module now**, or keep the profile in
   `conversations` until the assistant milestone? *Recommended:* the new
   module.
