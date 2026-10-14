## Why

The owner (2026-10-14): "the purpose of actions of email is that visor does
them for me, at least after the first. i have 1000 emails that look the same
and i said one should be moved to a certain place, all of them should be."
And: "rules should be translated into a smart rules system that become
deterministic."

Today a member's move to another stream (`conversations.move_to_stream`) only
records a correction. Nothing happens to the mail that looks the same, already
here or still to come. Corrections are written and never read; the k-NN
strategy (`campbooks_ai::decide::knn`, campbooks-cloud#83) runs only in the
eval harness, would cover only future mail, and is not explainable as a
sentence. There is no rule of any kind a member can see or remove.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/125.
Related: #83 (k-NN over corrections), #121 (free-text instructions, compiles
into the rules this change defines), #44 (automation), #89 (activity).

Modules touched:

- **`campbooks-ai`**: a pure, deterministic engine for taught rules
  (`decide::taught`) and the subject template normaliser, next to the email
  signals.
- **`conversations`**: owns taught rules (storage, previews, the decide-pass
  stage, application on arrival and in bulk, exceptions, undo). No new module,
  no new cross-module dependency, no `architecture.toml` entry.
- **`attention`**: understands a need cleared by a rule (cause `rule`).
- **Client**: the offer after an action, the bulk apply with progress and
  Undo, the "Visor moved this" line on rows and in the reader, the exception
  question, and a Settings → Scout's memory page.

## What Changes

- **One kind of rule.** A *taught rule* is a deterministic condition → action
  pair the workspace taught Campbooks, for one inbox. Conditions (all must
  hold): sender address, sender domain, List-Id, subject template, subject
  words, a stored header, a label, a stream. Actions: move to a stream,
  archive, add a label, mark read, "needs a reply" yes/no, "asks for an
  action" yes/no, always or never show. Every rule comes from somewhere:
  an action (teach once), an onboarding answer, an instruction in the
  member's words (#121), confirmed corrections, or a suggestion. All of them
  produce the same rule and are listed the same way.
- **Rules run first and always give the same answer.** In the decide pass
  taught rules are the first stage, ahead of the built-in signals and any
  model: same mail and same rules, same result. Their answers act (they were
  confirmed by a member) and are stored in `decisions` with source `taught`
  and the rule as the reason. A member's own action on a conversation still
  beats any rule; it is recorded as an exception. Mail that looks suspicious
  is never hidden, archived, marked read or moved out of Primary by a rule.
- **Teach once.** After a member moves, archives, labels, marks read or marks
  Not needed one conversation, Visor offers to do the same to its lookalikes:
  "Move all 1,000 like this to Updates, and new ones too?", with the
  likeness in plain words ("from notifications@github.com"), narrower
  choices with their counts, and a sample of real subjects. Ambiguity is a
  choice for the member, never a guess. One tap (Y) creates the rule and
  applies it.
- **Bulk apply with progress and one Undo.** A background run applies the
  rule to existing mail in batches, resumable and idempotent, with progress
  on the change feed ("Moving 1,000… 340 done"). Provider write-back goes
  through the existing outbound changes within the provider budgets. Undo
  (Z) removes the rule and puts back every conversation it changed that no
  member has touched since.
- **Visible after.** New mail a rule handled says so on its row and in the
  reader ("Visor moved this · like 1,000 others from notifications@github.com")
  with Move back and Stop doing this. Moving one back is an exception; the
  second exception within 30 days asks whether the rule is too broad and
  offers a narrower likeness.
- **Scout's memory.** A Settings page lists each inbox's rules as sentences
  ("Mail from notifications@github.com about "pull request" → Updates ·
  1,000 so far"), with where each came from, how often it acted, Edit and
  Remove (optionally putting the mail back). Rules waiting for a yes
  (proposed by #121 or by Scout from repeated corrections) wait there with
  their preview.
- **Operations** (`conversations:read` / `conversations:write`):
  `conversations.suggest_rules`, `preview_rule`, `create_rule` (undo:
  `remove_rule`), `confirm_rule`, `update_rule`, `remove_rule`, `list_rules`,
  `get_rule`, `dismiss_rule_suggestion`; event `conversations.rule_changed`
  v1; ADR 0017 resource type `conversations.rule` contained by the inbox.

## Non-goals

- Similarity (k-NN, embeddings) as a likeness. It stays #83's; later, its
  repeatedly confirmed suggestions are *proposed* as taught rules.
- Rules over other records (calendar, documents, money) and rules triggered
  by other modules' events: automation (#44) builds on this model later.
- Compiling free text or onboarding answers into rules (#121 owns that and
  calls this change's operations).
- Creating provider-side filters (Gmail filters, Zoho rules), deleting or
  trashing mail, starring, Later, Done on a need: none of these is a rule
  action. Done means "I did this one"; Later is personal.
- Rules spanning several inboxes as one record (a teaching spanning inboxes
  creates one rule per inbox sharing a `teaching_id`).
- The assistant activity page (#89); rules record their own applications,
  which #89 reads later.
