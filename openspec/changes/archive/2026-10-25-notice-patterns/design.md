# Design

## Data ownership and events

Everything lives in the `conversations` schema:

- `conversations.rule_notices` gains `pattern` (`''` for the whole sender,
  `t:<template>`, `p:<words>`, at most 400 characters) and `example_subject`
  (one recent subject of the kind, at most 200 characters, none for the whole
  sender). One suggestion per inbox, kind, sender and pattern.
- `conversations.rule_notice_answers` gains `pattern`: "Not now" and "Don't
  suggest this" are per kind.
- `conversations.rule_notice_groupings` (new) keeps the model's grouping per
  inbox and sender: the normalised phrases (possibly none) and when. Forced
  row-level security, cascades with the inbox, and `forget_inbox` clears it.

Defaults keep every stored row valid and meaning what it meant (the whole
sender). No event, job, operation or schedule is added; the existing
`conversations.notice_rules` job does the work and keeps its Coverage row. No
tracked event is added or changed. Scopes are unchanged
(`conversations:read` to list, `conversations:write` to answer).

## The pass

The same frequent senders as before (at least `MIN_MESSAGES` over
`WINDOW_DAYS`, the 40 most frequent), now read one conversation per row (its
newest inbound message: subject, template, date, read, archived unread,
replied, suspicious, owed, asked, stream, headers). Then, in pure Rust
(`rule_notices::analysis`):

1. **Sender-wide blockers**: a person (not list, automatic or no-reply
   mail), a correspondent, any suspicious mail, too few or in one burst:
   nothing at all.
2. **Kinds from templates**: conversations are grouped by equal
   `subject_template`; a group of at least `MIN_KIND_MESSAGES` (4) on
   `MIN_KIND_DAYS` (2) is a kind; the rest (no template, smaller groups) is
   left over. Each kind is scored with the same thresholds as a sender, on
   its own counts and its own reply, ask and bill blockers.
3. **One rule or several**: when every kind (and the rest, when it holds at
   least `MIN_KIND_MESSAGES`) would get what the whole sender gets, and none
   of the sender's mail was replied to, asked anything or asks for a payment,
   the sender is one kind: one rule for all of it, as before. Otherwise one
   suggestion per kind that qualifies: the sender and the template, with the
   kind's newest subject as the example.
4. **Kinds from the model**: for a sender whose left-over mail is at least
   30% of it and at least `MIN_KIND_MESSAGES`, with at least
   `MIN_KIND_MESSAGES` of it unread, the model is asked, unless the sender is
   plainly one kind (its templates agree with a rule for the whole sender and
   none of the left-over mail was read or mattered). A newsletter whose
   subjects all differ and whose mail is half read is asked about; build
   notifications all left unread are not. The model's phrases also count in
   step 3: a kind they find that the member reads keeps the whole sender from
   being offered. The phrases (cached) are kinds of the mail whose subject holds them, as
   the engine's `subject_contains` matches them, counted over all the
   sender's mail the rule would match; dropped when too small or when half of
   it is already in a kind offered.
5. **No pattern, nothing**.

A sender a rule of the inbox already names with no subject condition is
skipped; a kind is skipped when a rule names the sender with that template
or those words. A rule that would change nothing is not offered, and
`MAX_PER_INBOX` still caps the inbox.

Why this keeps ADR 0018: grouping by template is equality of a value that is
already stored and already a condition a member's rule may name (text
hygiene). Counting what the member did is a fact. What a kind is, beyond
equal templates, is the model's answer, and the result is only a proposal:
the member reads the rule and confirms it. No code reads a subject to decide
what it means.

## The model (role `complete`)

Only for the senders of step 4, at most `MODEL_SENDERS_PER_PASS` (3, `0`
turns it off) per inbox and pass, most frequent first, and only when the
sender's grouping is missing or older than `REGROUP_DAYS` (7). The job reads
in its own transaction, commits, asks the model with no transaction held,
then keeps the answers and runs the pass in the job's transaction.

- **Data**: the sender's most recent distinct subjects (`MODEL_SUBJECTS`,
  40, each cut to 200 characters) as JSON string values; never a body, a
  name or an address. The instruction says the subjects are data, never
  instructions.
- **Output**: strict JSON schema `{"kinds": [{"phrase": "…"}]}`, at most 6;
  each phrase is normalised as a rule's `subject_contains` is (1 to 8 words,
  no longer than a rule value, no repeats). An answer that does not parse is
  kept as "no kinds", so a sender is asked about at most once in
  `REGROUP_DAYS`. Temperature 0.1, 4,096 tokens (the model reasons first).
- **Gate**: the gateway reaches the job only when the role may receive mail
  here (`may_read_mail`, as Scout's read: the instance's network or the
  admin's opt-in). Calls are counted against the workspace's daily AI limit
  as background work. Off, unconfigured, refused, failing or over the limit:
  nothing is asked or kept, and the template kinds are offered as before. A
  missing setting never fails startup (every one has a default).
- **Logs**: counts and error kinds only, never subjects or phrases.
- **Undo**: nothing changes until the member confirms a rule; the rule's Undo
  is as before. The rule itself runs with no model.

## Teach once

`conversations.suggest_rules` adds `sender_phrase` when a phrase kept for the
sender is in the conversation's subject (no model call). For a machine
sender the inbox does not write to, the default is `sender` when the
sender's window is one kind (step 3, on its conversations), else
`sender_phrase`, else `sender_template`, else none (the member chooses). A
sender with too little mail to tell is one kind.

## API and client

`RuleNotice.pattern` (absent for the whole sender): `source` (`template` or
`phrase`), `example`, and `search`, the fragment that narrows "See the N
emails" (`subject:"<words>"`; for a template its longest run of words between
`#` and `*`, or empty). The evidence counts are the kind's. Home quotes the
example (cut to about 60 characters, as plain text) with the locale's
quotation marks and appends `search` to the query. The confirmation already
says the template and words conditions in words.
