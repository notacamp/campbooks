# Design

## Data ownership

Everything stays in the `conversations` schema. `conversations.needs`
accepts `question = 'money'` and `verdict IN ('already_paid',
'still_to_pay')` (checks widened `NOT VALID`, validated in a migration of
their own). The money answer is stored in `conversations.decisions` like
every triage answer (`question = 'money'`, a label value). Corrections keep
their shape: `money` from `owed` to `owed` (Still to pay) or `not_owed` (Not
needed). No event changes: `conversations.need_changed` v1 already carries
`reason: pay` and `trust: check`; Already paid clears with cause `done`.

## The money question (role `decide`)

`money` is a `choice` with three options, asked with the other triage
questions in the one decide call per inbound message (Jev answers a call in
about 0.4 s; one more question adds no call). Without a trusted `decide`
role nothing changes: no model answer, no Pay need, as today. The triage
context and the wording of the existing questions are unchanged, so
`CONTEXT_VERSION` stays 2; answers to `money` are identified by their
question id.

## Raising Pay

`raise()` (pure) reads the stored answers:

1. A suspicious message is a warning, as before.
2. A money answer that is `owed` counts when the policy let it act, or when
   it is the model's at `CHECK_FLOOR` (0.8) or more; an acted money answer
   saying `paid` or `neither` wins. When one counts and the message is not
   in Newsletters, the need is `pay`, trust `check`, question `money`.
3. Otherwise a message in Receipts needs nothing (unless a member's taught
   rule says it needs a reply or an action).
4. Reply, Do and the Review questions as before.

Settling: `already_paid` clears as `done`; `still_to_pay` (like `confirmed`)
makes the need sure with basis `member`, and keeps it when its message is
read again.

## Endpoint

`POST /api/v1/w/{workspace_id}/conversations/{conversation_id}/need/resolve`
(`conversations.resolve_need`, scope `conversations:write`, role `sender` or
above) gains the outcomes `already_paid` and `still_to_pay`. Undo is
`conversations.reopen_need`, unchanged.

## Policy

`money` records (no `ActAbove` entry) until an eval of the owner's labelled
mail gives an operating point. The policy test pins every measured question
to the committed report and every unmeasured one to `Record`.

## Upgrade and backfill

The migration's seed queues every inbox for `conversations.backfill_needs`.
The backfill's model-pass condition becomes "the newest recent inbound
message lacks a checked model answer to any current triage question", and
`conversations.decide_backlog` skips a message only when every question has
a checked answer (a live `decide_message` still skips a message with any
checked answer: a replayed delivery). Re-asking keeps the stored checked
answers (the upsert never replaces them) and adds `money`. Between the
backfill's refresh and the model's answer an old Pay need is withdrawn as
`superseded`, then raised again as a question if the money answer is `owed`.

## Labelling

`campbooks-server labelling export` runs on the server with shell access, on
the runtime login inside the workspace (row-level security applies), reading
only stored messages and their text blobs. It writes `manifest.json`
(private set `owner-money`, question `money`), `items/owner.jsonl` (the eval
item format; `dev`/`test` alternate) and `labels.csv`. The xtask eval loads a
`private` set from `--set-dir` without the fictional set's reserved-domain
and id checks, fills `labels.money` from the sheet, leaves unlabelled items
out of the scores, and writes its report into the folder. Nothing of it is
committed or sent anywhere but to the configured `decide` endpoint the
instance already uses.
