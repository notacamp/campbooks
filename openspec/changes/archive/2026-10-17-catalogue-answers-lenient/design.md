# Design

## Where the leniency lives

Two places could absorb a bad catalogue answer:

1. **The parsers** (chosen). `laya::into_raw` and `openai::into_raw` already
   walk the questions one by one. For a catalogue id (`action_*`, the ids in
   `signals::actions::question::ALL`) a missing key, a value of the wrong
   shape or one that does not decode to the question's type yields `None`
   instead of an error. Every other id keeps today's error. The gateway
   drops the unanswered ones from `Answers`, and the model strategy maps
   answers by id, so an unanswered question is an abstention and the triage
   positions never shift.
2. **A second, triage-only call** when the first one fails. Rejected: it
   doubles cost and latency exactly when the model is misbehaving, and
   still loses every rating of that message.

Laya's response now decodes `answers` as JSON values and each answer on its
own, so a malformed triage answer fails with "answer `<id>` is malformed"
rather than on the whole body. It is still a failed call, as before.

## Why "NaN" is tested as a string

JSON cannot carry NaN; a model that means it sends a string or a number out
of range. The tests cover a missing key, `"NaN"`, a value out of the
`noul` range and, over OpenAI, a non-boolean, on both protocols, and check
that triage decisions match the catalogue-off run.
