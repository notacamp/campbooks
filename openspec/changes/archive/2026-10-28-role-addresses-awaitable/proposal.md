# Role addresses can be awaited and named sensibly

Modules: `campbooks-ai` (`decide` facts: who may be awaited, a display name)
and `conversations` (the names the needs carry). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/379. No new module and no
cross-module dependency. Builds on whose move it is (campbooks-cloud#293) and
"Scout asks when it is unsure" (#372).

## Why

1. When someone writes to `info@company.example` and waits for an answer,
   that mailbox is exactly who to nudge. `awaitable` dropped it, because it
   dropped every address that looks like a service's. That list exists to
   spot machine senders, not people who cannot answer.
2. A shared mailbox shows as "Geral", "Info" or "Support". The card read
   "Nudge Geral...".

## What changes

- **Role addresses stay awaitable.** On the To/Cc side only addresses that
  cannot answer are dropped (no-reply, bounce, mailer-daemon, postmaster),
  with the inbox's own addresses, the sender and duplicates.
- **A sensible name.** The server names each person on a question (`unsure`)
  and each person a wait is on (`waiting_on`): the header's display name when
  it is not a role word, else the organisation read from the registrable
  domain, else the address.
- A machine sender still means nobody is awaited (`is_machine_sender` is
  unchanged).

## Non-goals

- The model's prompt, the policy thresholds, the action catalogue.
- Teach once (#378).
- Putting the organisation name in the model's option descriptions or in the
  recipient's name of a nudge draft: both keep the header's name.

## Impact

Doc comments of `Unsure.others` change (OpenAPI and client types follow). No
migration, no endpoint, no scope, no tracked event.
