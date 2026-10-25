# Proposal

## Why

Scout noticed and teach once treat a sender as one thing. One reply, ask or
bill anywhere in a sender's mail blocks every offer for it, and the opened
rate is averaged over all of it. So a shop whose promotions the member
ignores but whose shipping notices they read gets either nothing or "keep
everything off Home", which hides the shipping notices too.

The owner, 2026-10-25: "the smart rule suggestion based on sender is a bit
too simplistic, sometimes there are more patterns or none for the same
sender. we should try to find patterns in content for frequent senders." The
owner approved all three steps below in one change.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/356

## What Changes

- **Kinds from templates, without the model** (`conversations`): a frequent
  sender's conversations are split by their subject template, which is
  already stored and already a rule condition. Each kind is counted on its
  own, and a reply, an ask or a bill blocks only its own kind. A person, a
  correspondent and any suspicious mail still block the whole sender. A
  kind's rule is the sender and that template.
- **Kinds from the model, where templates cannot split** (`conversations`,
  `complete` role): for a mixed sender whose mail mostly fits no template, at
  most three senders per inbox and pass, the model sees that sender's recent
  subjects only and proposes kinds, each as a run of words. Every proposal is
  tested against the sender's mail with the rule engine's own
  `subject_contains` matcher and the same counts; what fails is dropped. The
  answer is kept for seven days. The saved rule runs with no model. The
  content opt-in and the daily AI limit apply.
- **No pattern, no offer**: a mixed sender with no kind to offer gets
  nothing, never a rule for all its mail.
- **Teach once** (`conversations.suggest_rules`): a new likeness,
  `sender_phrase` (the sender and the model's words kept from the pass, never
  a new call). A machine sender's default is all its mail only when that mail
  is all one kind; else the phrase, else the template, else the member
  chooses.
- **API**: a suggestion gains `pattern` (`source` `template` or `phrase`, an
  `example` subject, a `search` fragment), and its counts are the kind's.
- **Home** (`conversations` client): the note names the kind by a real
  subject ("Northwind sent you 40 emails like “Deals of the week 40” …"), and
  "See the N emails" narrows the search to it.
- A migration adds `pattern` and `example_subject` to suggestions, `pattern`
  to answers (each kind is answered on its own), and the model's cache table.

## Non-goals

- No body, name or address goes to the model; no new `decide` question.
- No phrase list, keyword list or regular expression over subjects: kinds are
  template equality or the model's words, and the member confirms the rule.
- No change to `campbooks_ai::decide::signals::facts`.
- No new endpoint, operation, job or event; no new tracked event.
- No model call when teach once is asked: it only reads what a pass kept.

## Impact

- Modules: `conversations` (server and client), `campbooks-ai` (the engine's
  `subject_contains` test made public, unchanged).
- No new module and no cross-module dependency.
