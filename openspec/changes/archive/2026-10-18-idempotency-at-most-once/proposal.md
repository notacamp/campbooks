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

- **Time limit and disconnects.** A request stopped at the server's 30-second
  limit, or one whose client or proxy hung up, also ends without a stored
  answer, so its key gives "outcome unknown" too. The time limit now answers
  503 `request_timed_out`, its own problem code, in the caller's language,
  saying to check and then retry with a new key. Before, it was a bare 503
  that read like a server error, which suggested the key had been released;
  it never was. Only API clients send keys today; the web app does not.
- **What stays as before.** A 5xx the operation itself answers releases the
  key. An operation that committed before such an error, or before a lost
  acknowledgement, can run again on retry unless it has its own guard (mail
  has the draft's send state). The spec states this, and it is filed as a
  follow-up.

## Impact

- Spec: operation-catalogue (Idempotent mutating operations).
- Code: kernel `idempotency` (`claim_in`: no takeover, `Claim::Unknown`),
  server `time_limit` (503 `request_timed_out`, replacing tower's
  `TimeoutLayer`), four kernel locale files.
- No migration: the schema is unchanged.
- Behaviour: a request whose process died now needs a new key (after
  checking whether it took effect) instead of being re-run.
