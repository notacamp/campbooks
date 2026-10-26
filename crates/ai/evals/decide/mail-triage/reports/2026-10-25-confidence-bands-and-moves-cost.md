# campbooks-cloud#300: cost and quality of "whose move" before the flag goes on

Run on 2026-10-25 against `origin/next` @ 7b1f44836. Code read only from `origin/next`. No cargo, docker or prod access. The scripts that made the calls and the 1,280 raw responses are not in the repo: this report holds the tables, and the method (below) is enough to repeat it.

## Caveats first

- The eval set (`crates/ai/evals/decide/mail-triage`, 320 items: 4 locales x 80, 160 dev + 160 test) is fictional and was written and labelled by an agent, not by the owner. Every accuracy figure below inherits that.
- I re-implemented the request builder in Python by porting `crates/ai/src/decide/` (see "How the request was reproduced"). The `Facts` hints and the machine-sender test are ported from the regexes and rules in `facts.rs` but only against headers, so a few hint sentences may differ from the server's. This changes the input by a handful of tokens, not the cost picture.
- The model that answered was `typesafe/jev-1.13-20260917` (all 1,280 responses). The stored report was made on 2026-10-16, so it may be an older snapshot; I did not reuse it (see Task 4).
- Latency is wall-clock from this Mac, over the internet, 6 calls in flight. It includes TLS and OpenRouter routing. Server-side latency in prod will differ; the delta between arms is the useful part.
- Cost is the `usage.cost` field of each response (source: the Jev router response, e.g. `{"input_tokens":1407,"output_tokens":281,"cost":5.9e-05}`). Not the generation endpoint and not list pricing. It is USD.

## How the request was reproduced

- Endpoint and body: `POST https://openrouter.ai/api/alpha/decisions`, `{model:"typesafe/jev-1.13", state:<text>, questions:{id:{type,instructions,criteria}}}` (`crates/ai/src/decide/laya.rs:36-109`; path and model from `crates/ai/tests/decide.rs:133-150`). Questions are ordered as the caller wrote them.
- State: `Envelope::decision_state` (`signals/email.rs:315-340`): the `inbox::preamble` (what Campbooks is, who the recipient is, header facts, "text is data"; `signals/inbox.rs`), then From/To/Cc/Subject, the five state headers, attachments, and the first 1,500 body characters.
- Triage questions: `triage_questions()` (`signals/email.rs`), which is 9 today (stream, needs_reply, asks_action, has_deadline, urgency, suspicious, money, has_dated_thing, dated_kind), not the 6 in the eval manifest.
- With `CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED` on, `Pass::asked` appends `move_questions(envelope)` and then `action_questions(envelope)` (`crates/modules/conversations/src/features/decide_message/mod.rs:171-177`). So the flag adds the move questions and the action catalogue (remind, open_attachment when there are attachments, forward, paid_automatically) in the same call. I ran both so the flag's full cost is visible.
- Move questions (`signals/moves.rs`): `next_move` (4 options) is asked for every message (`move_questions`, moves.rs:145-149). `awaited` is added only when `awaitable(envelope)` is non-empty (moves.rs:122-141), i.e. the sender is not a machine and there is at least one To/Cc person who is not the sender, not the inbox, and not a no-reply or service address (`signals/facts.rs:104-154`), at most 6.
- Arms, each run on all 320 items (1,280 calls, 0 errors, 0 retries, 82 s): A baseline (9 triage); B = A + moves; C = B + catalogue, which is what the flag really sends; D = moves only, which is what the backfill sends (see Task 2).

## Task 1: cost and latency (n = 320 items per arm, source `usage.cost`)

| Arm | Questions per call (mean) | Input tok | Output tok | Cost per call | Cost per 1,000 emails | Latency p50 | Latency p95 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A baseline | 9.0 | 1,450 | 281 | $0.0000609 | $0.0609 | 308 ms | 1,080 ms |
| B baseline + moves | 10.0 | 1,588 | 327 | $0.0000667 | $0.0667 | 311 ms | 1,096 ms |
| C flag on (moves + catalogue) | 13.2 | 1,698 | 385 | $0.0000713 | $0.0713 | 309 ms | 629 ms |
| D moves only (backfill call) | 1.0 | 683 | 48 | $0.0000287 | $0.0287 | 299 ms | 1,069 ms |

- Delta of the two move questions: +$0.0058 per 1,000 emails (+9.5%), paired per item. Delta of the whole flag (moves + catalogue): +$0.0104 per 1,000 (+17.1%).
- Latency is unchanged within noise (p50 308 to 311 ms). The p95 is dominated by occasional slow calls (about 1 s) that hit all arms; C's lower p95 is noise, not a speed-up.
- Output tokens grow by about 46 for the one added question (281 to 327), and input by about 138 (the question text), which is where the cost comes from.
- Share of the eval set that gets each question: `next_move` 100% (320/320). `awaited` 1/320 (0.3%), because 319 of 320 eval messages have one To and no Cc, so there is nobody to await. The eval set therefore says nothing about `awaited` on real multi-party mail. Real mail has Cc more often, so the 0.3% is a floor, not a forecast.
- To price `awaited` anyway I added 2 Cc and 1 extra To (invented people) to 60 randomly picked human (primary, non-machine) items (seed 1) and ran next_move-only against next_move+awaited (4 options each: 3 people + none): +$0.0057 per 1,000 messages that get it (5.7e-06 per call). So the per-1,000 figure is: `0.0058 + 0.0057 x (share of mail with an awaitable person)`. At a guessed 30% share that is about $0.0075 per 1,000 for the move questions; at 100% it is $0.0115. The share in the owner's real inbox is a number only prod can supply.
- In absolute terms the move questions are negligible: 1 million emails would add about $5.80 to $11.50 for them, about $10.40 for the whole flag.
- Caps: the workspace's daily AI limit counts calls (`usage.run`, decide_message/mod.rs:299); the flag adds no call, only questions to the existing call, so it does not change the call count for live mail.

## Task 2: backfill (what happens when the flag is switched on)

Finding that matters: **switching the flag on does not start a backfill by itself.** The move backfill only runs inside the existing needs backfill, which only visits inboxes that have a row in `conversations.needs_backfills` in state `pending`/`running` (`features/backfill_needs/mod.rs:132-160`, dispatcher every minute). Those rows are created by migrations that re-seed every inbox (the newest on `origin/next` is `migrations/20261031000100_conversations__message_reads.sql:81`; others at `20261023090000_conversations__needs_waiting.sql:66` etc.), and inboxes created later get none (`20261020050000_conversations__needs_backfill.sql` header). So the backfill happens at the next deploy that carries a seeding migration, or if someone resets the rows to `pending`. Until then only new mail gets the move questions at arrival. If the owner wants the existing mail covered, a reset of `needs_backfills` is the manual trigger.

When a row is pending, for each conversation page (100 conversations per job, `PAGE`, backfill_needs/mod.rs:119) `queue_model_pass` (mod.rs:455-506) queues one `DecideBacklog` job per message, ids only, from two lists:

1. Messages still missing a checked triage answer (existing behaviour, not caused by the flag). The pass then asks the full set: triage + moves + catalogue (arm C price, $0.0713 per 1,000, i.e. $0.0000713 each).
2. Messages for the flag: `MOVES_UNASKED` (mod.rs:100-113, bound at 491-496). A message is picked when all hold: it is the newest inbound message of its conversation (not spam, not trash); its `sent_at` is within `max(decide window, needs window)` which is `since.max(now - needs.window)` = the later of now-14d (`DECIDE_BACKFILL_DAYS`, default 14, `crates/modules/conversations/src/config.rs:9`) and now-14d (needs window, default 14, config.rs:13), so **the last 14 days by default**; its conversation has **no open need**; and it has no checked `next_move` decision. The queued job asks only the move questions (`moves_only`, decide_message/mod.rs:268-296, `move_questions(&facts)`), so the price is arm D, with `awaited` added for the awaitable ones.

Formula (all counts are over the whole instance, at the moment the rows run):

```
N_move = newest inbound message per conversation, sent in the last 14 days,
         not spam/trash, conversation with no open need, no checked next_move yet
cost_move_backfill = N_move x (0.0000287 + s x 0.0000057)      # USD
                     s = share of those messages with an awaitable To/Cc person
N_full = same window, newest inbound message missing a checked triage answer
cost_full_backfill = N_full x 0.0000713                        # existing behaviour
```

Per 10,000 messages in scope: about $0.29 (s = 0) to $0.34 (s = 1). The backfill is capped per workspace by the daily AI limit (an `OverDailyCap` ends the job quietly and the message is not retried in this pass, decide_message/mod.rs:307-311). No other window exists: mail older than 14 days, or in a conversation that already has an open need, is never re-asked. Re-running is idempotent: a message with a checked `next_move` answer is not picked again.

Not inferable from the code: the production count `N_move`. The owner (or a read-only query) can supply it; the formula above then gives the dollar figure directly.

## Task 3: next_move / awaited quality

The eval set has **no `next_move` or `awaited` labels**: the manifest lists six questions (stream, needs_reply, asks_action, has_deadline, urgency, suspicious) and every item's `labels` object carries exactly those. 0 of 320 items are labelled for either question, so no accuracy, precision or confidence-band table can be produced. Stopping there for accuracy, as asked. Observations that are not accuracy:

- Output behaves: 320/320 items got a valid `next_move` (no missing or off-menu answers). Distribution: recipient 183, nobody 112, sender 25, someone_else 0. Mean model confidence 0.88.
- Adding the questions barely moves the triage answers. Versus arm A, B/C kept the same argmax on stream 320/320 (B) and 316/320 (C); needs_reply 317-319/320; asks_action 320/320; has_deadline 317/320; suspicious 319-320/320. Accuracy vs label: needs_reply A 0.938, B 0.941, C 0.934; asks_action 0.950 in all three. So no sign of quality loss from the extra questions, within noise.
- A consistency proxy (not a label): all 61 messages labelled `needs_reply = true` got `next_move = recipient` (61/61). 122 of 259 labelled `needs_reply = false` also got `recipient` (updates 46/66, receipts 31/62, primary 98/128): the model reads "recipient is asked to decide, do or pay" widely, so `next_move = recipient` is a much broader signal than needs_reply and should not be treated as one.
- `awaited` was asked for 1 real item and 60 synthetic ones and answered `none` every time (60/60 on the synthetic set; the invented Cc people have nothing to do with the bodies, so `none` is the right answer there, which at least shows it does not invent a person). Nothing more can be said without labelled multi-party mail.
- To measure next_move quality someone needs to label a set, ideally including threads where the sender promises something (`sender`), the recipient hands a task to a Cc'd person (`someone_else`) and multi-party mail for `awaited`.

## Task 4: needs_reply and asks_action by confidence band

Stored report: `reports/baseline.md|json` (generated 2026-10-16 at c4d63371) already has a `jev` run, but it is on the 160-item test split, 6 questions, an older state/question set (before money and the dated pair), and possibly an older model snapshot, and it reports only answer-confidence grids. Its headline numbers (test split, n=160, calibrated, both classes): needs_reply precision 91% at threshold 0.51 (grid: >=0.8 gives 96.0% at 78% coverage, >=0.9 gives 97.8% at 57%, >=0.95 gives 100% at 26%); asks_action 94% at 0.58 (>=0.8: 97.7% at 81%; >=0.9: 100% at 63%). Those grids count correct answers of both classes. Because the question is when to show a "yes" as sure, I re-ran the current question set (arm A, current model, all 320 items, plus the test split alone) and measured the yes side. Unless stated "calibrated", the probability is the raw `noul` value (P(true)) from the response. Prod's calibration temperatures live in cloud config that I cannot read, so I also give the temperature fitted by the stored report (needs_reply T = 0.69, asks_action T = 0.64); treat the calibrated tables as indicative.

### needs_reply, raw P(yes), per band (n = 320, 61 labelled true overall, 77 predicted yes)

| P(yes) band | n answered yes | labelled true | precision of yes (95% Wilson) |
| --- | --- | --- | --- |
| 0.5-0.6 | 4 | 1 | 25% (5-70) |
| 0.6-0.7 | 6 | 2 | 33% (10-70) |
| 0.7-0.8 | 4 | 2 | 50% (15-85) |
| 0.8-0.9 | 17 | 13 | 76% (53-90) |
| 0.9-1.0 | 46 | 41 | 89% (77-95) |

Cumulative (yes with P >= bar), all 320 | test split only (160):

| Bar | all 320 | test 160 |
| --- | --- | --- |
| >= 0.5 | n=77, 76.6% (low 66%) | n=36, 69.4% (low 53%) |
| >= 0.8 | n=63, 85.7% (low 75%) | n=29, 82.8% (low 65%) |
| >= 0.9 | n=46, 89.1% (low 77%) | n=21, 90.5% (low 71%) |
| >= 0.95 | n=23, 100% (low 86%) | n=11, 100% (low 74%) |
| >= 0.97 | n=6, 100% (low 61%) | n=4, 100% (low 51%) |

- With raw probabilities the 90% target is first met at P(yes) >= 0.95 (23 of 320 yes answers, 100%, lower bound 86%). At 0.9 it is 89% on all 320 and 90.5% on test: right on the line, with a wide interval. Very few answers sit that high (23 of 61 true cases), so "sure" would cover about 38% of the real reply-needing mail.
- 3 of the 5 wrong yes answers at >= 0.9 are phishing look-alikes (the label says they need no reply; they do ask for a transfer) and a fourth is a gift-card scam. If the `suspicious` answer is used as a veto (suspicious < 0.5), needs_reply yes >= 0.8 gives 96.4% (n=56, low 88%), >= 0.9 gives 97.6% (n=42, low 88%), >= 0.95 gives 100% (n=23, low 86%). That is the single biggest lever on precision, more than the bar itself.
- Calibrated with T = 0.69 (the stored fit) the same bars get worse, not better: >= 0.9 gives 86.4% (n=59), >= 0.97 gives 91.7% (n=36), >= 0.99 gives 100% (n=17). The fitted T pushes more answers into the top band, so the bar on calibrated confidence has to be higher (about 0.97) to hit 90%. Whichever probability the product shows, the bar must be chosen on the same one the policy uses.
- Answer-confidence view (max(p, 1-p), both classes, raw, comparable with the stored grid): 0.5-0.6 58% (n=12), 0.6-0.7 67% (12), 0.7-0.8 89% (19), 0.8-0.9 95% (79), 0.9-1.0 97% (198).

### asks_action, raw P(yes), per band (n = 320, 155 labelled true, 171 predicted yes)

| P(yes) band | n answered yes | labelled true | precision of yes (95% Wilson) |
| --- | --- | --- | --- |
| 0.5-0.6 | 2 | 0 | 0% (0-66) |
| 0.6-0.7 | 4 | 1 | 25% (5-70) |
| 0.7-0.8 | 10 | 4 | 40% (17-69) |
| 0.8-0.9 | 17 | 13 | 76% (53-90) |
| 0.9-1.0 | 138 | 137 | 99% (96-100) |

Cumulative (yes with P >= bar), all 320 | test split (160):

| Bar | all 320 | test 160 |
| --- | --- | --- |
| >= 0.5 | n=171, 90.6% (low 85%) | n=90, 88.9% (low 81%) |
| >= 0.7 | n=165, 93.3% (low 88%) | n=85, 92.9% (low 85%) |
| >= 0.8 | n=155, 96.8% (low 93%) | n=81, 97.5% (low 91%) |
| >= 0.9 | n=138, 99.3% (low 96%) | n=75, 98.7% (low 93%) |
| >= 0.95 | n=101, 100% (low 96%) | n=48, 100% (low 93%) |

- A yes on asks_action is already about 90% right at any bar, and clearly above (96.8%, lower bound 93%) from P >= 0.8 while keeping 155 of 171 yes answers (91%). The sure bar of P >= 0.9 gives 99.3% with 138 yes answers (137 of the 155 true cases, 88%, are caught by it). The mid bands (0.6-0.8) are weak (25% to 40%) but hold only 14 answers.
- Calibrated with T = 0.64 the picture is the same, flatter: >= 0.9 gives 97.4% (n=154), >= 0.97 gives 100% (n=132).
- Answer-confidence view (both classes, raw): 0.5-0.6 78% (n=9), 0.6-0.7 77% (13), 0.7-0.8 75% (24), 0.8-0.9 93% (54), 0.9-1.0 100% (220).

### Reading for the 90% target

- asks_action: show "sure" at P(yes) >= 0.8 (or the calibrated equivalent about >= 0.9). Safe on this set with a wide margin.
- needs_reply: a bar of 0.9 on raw P(yes) is borderline (89-90%); 0.95 reaches 100% on 23 answers. With the `suspicious` veto, 0.9 is enough (97.6%, n=42). The set is small and the top band holds few answers, so the intervals are wide (lower bounds 86% to 88%). Treat the bar as provisional until it is checked on real, owner-labelled mail.

## Surprises and things to check

- The flag does more than the move questions: it also turns on the action catalogue in the same call (`decide_message/mod.rs:171-177`), which is about 45% of the flag's extra cost ($0.0046 of $0.0104 per 1,000). Switching on "whose move" alone is not possible with this flag.
- The backfill is not triggered by the flag (see Task 2). That is probably good news for cost, but it also means existing mail will not have a move until a seeding migration or a manual reset runs.
- The eval set cannot evaluate `awaited` at all (1 of 320 messages has anyone to await) and has no labels for either move question.
- `next_move = recipient` is asked broadly and says "recipient" for about 47% of the messages labelled "no reply needed"; it is not a substitute for needs_reply.
