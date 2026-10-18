## Why

The idempotency layer claims a key, runs the operation, then stores its
answer, each in its own transaction (campbooks-cloud#63). A process that
stops after the operation committed and before the answer was stored leaves
the key pending. Two minutes later the layer took that key over as
abandoned and ran the operation again: the same send, change or invitation,
twice.

## What Changes

- **A pending key is never taken over.** A key is taken only when it is new
  or expired.
- **Its outcome is unknown.** While it has been pending for less than two
  minutes (longer than any request runs), a retry gets 409
  `idempotency_key_in_flight`, as before. After that it gets 409
  `idempotency_outcome_unknown` ("may have been done; not done again; check,
  then use a new key") until the key expires (24 hours). The message is in
  all four locales.
- **Why not one transaction.** Storing the answer in the operation's own
  transaction would need every operation to run inside one transaction the
  layer owns. Operations use several transactions, call providers, enqueue
  jobs and run under different database scopes (workspace, account, admin,
  kernel), so the layer cannot own them. Hooking only the kernel's workspace
  transactions would leave identity's and workspaces' operations unguarded.
  The guarantee that matters, that an operation runs at most once per key,
  holds for every operation this way, with no change to any handler.
- **Unchanged:** an identical retry after an answer replays it;
  `secret_once` answers are withheld; server errors and authentication or
  rate-limit failures release the key; and row-level security keeps keys to
  their caller.

## Impact

- Spec: operation-catalogue (Idempotent mutating operations).
- Code: kernel `idempotency` (`claim_in`: no takeover, `Claim::Unknown`),
  four kernel locale files.
- No migration: the schema is unchanged.
- Behaviour: a request whose process died now needs a new key (after
  checking whether it took effect) instead of being re-run.
