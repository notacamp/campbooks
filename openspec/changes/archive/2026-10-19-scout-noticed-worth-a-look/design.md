# Design

## Data ownership and events

- `conversations` owns the rule action, the need and its question
  (`conversations.rules`, `conversations.needs`, `conversations.decisions`).
  `attention` owns the item (`attention.items`) and only gains a reason. Both
  are widened by migrations that touch their own schema (`NOT VALID`, then a
  second migration validates).
- `conversations.need_changed` v1 keeps its shape; `reason` gains
  `worth_a_look` and `cleared_because` gains `opened` (contract and
  `openapi/events.json` updated). Older consumers see new enum values; there
  are none outside `attention`.
- No endpoint is added. `conversations.create_rule` already accepts any valid
  actions with a `notice_id`, and `check_open` only tests that the notice is
  open and belongs to the inbox, so a goal other than Scout's guess needs no
  server change (a new test locks that in). Scope unchanged:
  `conversations:write`.
- No AI role is added. The free-text goal calls `assistant.compile_instruction`
  as Scout's memory does (the role that operation already uses; nothing new is
  asked of an instance). Without a model the endpoint answers
  `compile_unavailable`; the confirmation then says so and the other cards
  stay. The result is a proposal the member reads in the confirmation, and
  Undo removes the saved rule.

## The action

`worth_a_look` fills a new engine slot, `highlight`, so one rule cannot say it
twice, and it is refused with anything that hides mail (`archive`, `mark_read`,
`trash`, `spam`, `mark_paid`, `surface: never`, "no reply" / "no action", a
move out of Primary): "I like these" and "get these out of the way" in one
rule is a mistake worth a 400, not a precedence puzzle. `evaluate` returns
`Outcome::highlight`; `answers()` stores it as a `taught` decision under the
question `worth_a_look` with value `true`, through the same path as `surface`
(on arrival, over existing mail, and undone with the rule's `answered`
questions).

It is not `surface: always`: that answer keeps every other rule from hiding the
mail. A highlight changes nothing about the mail, so other rules (including
Archive by hand) act as usual.

## The need

One need per conversation stays true. `raise` returns a `worth_a_look` need
only when nothing else would be raised (a warning, a reply, a task or a bill
outranks it), and `surface: never` wins over it (`taught_first` returns none).
It is `sure`, basis `taught`, not urgent, never a question.

`settle` gains `Since::opened` (the need's message is read). A `worth_a_look`
need with `opened` clears as `ClearCause::Opened`; no other reason is touched
by it. The read bit is a plain read of `conversations.messages.read`, which the
sync already keeps current (`apply_changes` updates it when the provider says
so), so reading on a phone or another client clears the item as well.

`conversations.mark_read` and `mark_unread` passed no need settings and so did
not work the need out again; they do now (`Extension<NeedSettings>`, as
`archive` does). Marking unread again re-raises the item: the answer is still
there.

A member's Done is "Seen" in the UI; the server files no correction for a
`worth_a_look` verdict (there is no model answer to correct). Not needed is
not offered by the client and, if sent, acts like Done.

Done today does not list `opened` items (the SQL excludes the cause, as for
`rule`), so the list is not filled by the act of reading.

## Home

- `Reason` gains `worth_a_look`, ordered after `pay` in the attention list.
- `signalOf(reason)` maps it to the `look` signal; `lib/ui` gains the hue
  `--signal-look-h`, the colour `--signal-look`, its `data-signal` binding and
  the `Signal` entry (tokens only, in all four locales for the Pill label).
- `kindOf` returns `highlight` for the question, `rowActionIds("highlight")`
  is `done` and `snooze`; server-ranked actions are ignored for it.
- Scout noticed is a child of the list container, rendered once. The first
  group is followed by it, or it follows the list when there is none. When the
  first group appears or disappears while the confirmation is open, the
  component remounts (its dialog closes): accepted, rare, and the draft is
  cheap to rebuild.

## The confirmation

`SmartRuleDraft.goal` (present for a notice) turns on the first question.
`Confirm` keeps the picked goal and, for "as you said", the compiled actions;
the actions it previews, sentences and saves are derived from them. With no
goal picked the preview query is disabled and Save is disabled.

The compiled proposal's conditions are discarded on purpose: the sender is the
notice's, already shown to the member; only the actions of the first proposal
are used (the compiler's actions are a subset of the rule actions). A sentence
whose outcome is not `rules`, or that has no actions, says so plainly.

## Read state ("also check")

The issue asks whether a provider's read state syncs back, since the evidence
("archived or deleted unread") would otherwise be wrong for a member who reads
on another device. It does: `apply_changes` updates `messages.read` whenever
the provider's flag differs (`was.read != message.read`), and the notice
analysis counts `read` from the table ("opened, here or at the provider"). What
it cannot see is a message read and archived between two syncs on a provider
that reports the final state only, or one deleted at the provider (the row is
removed, so it counts as neither). Nothing is changed here.

## Alternatives considered

- A separate "highlight" flag on the conversation instead of a need: rejected;
  the attention queue, Later, Done today, the change feed and the group order
  are all needs, and a second mechanism would repeat them.
- Reusing `surface: always`: rejected, it pins and blocks every other rule.
- Clearing on "opened in the app" only: rejected; opening on a phone should
  count, and the read bit is the one fact that covers both.
