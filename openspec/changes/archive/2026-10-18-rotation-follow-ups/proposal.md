## Why

The re-review of the instance secret key rotation (public #589,
campbooks-cloud#191) left three follow-ups (campbooks-cloud#262):

- **Outbound changes are not re-tagged.** A change waiting to be made at a
  mail provider carries a MAC under the key. If its stream stayed paused (or
  the provider down) for the whole rotation, it would end `skipped`
  (`unverified`) once the previous key was removed. That is safe, because
  the message is read again, but the member's change is lost.
- **Only rules and applications are tested end to end.** A drift in another
  kind's field rebuild would be skipped silently and not counted in `left`.
- **A doc comment** said the first run comes within 15 minutes of a start;
  it is at start-up.

## What Changes

- **Connectors rekey job.** `connectors.retag_bindings`, registered with
  `Background::rekey`, re-tags every pending outbound change whose MAC
  verifies under the previous key only (`domain::outbound::retag`). Each
  update is a compare-and-set, and it logs what is left, like
  `conversations.retag_bindings`.
- **Per-kind tests.** Each kind is signed under the old key, re-tagged, and
  then used under the new key alone through its real verifying path:
  - a running rule run works through its mail;
  - a change request stands at the outbound port;
  - a scheduled send is released;
  - a stored send stands;
  - a pending outbound change is made.
- **The doc comment** on `REKEY_EVERY`.
- **The runbook** names outbound changes and the second log line.

## Impact

- Spec: web-bff (the instance secret key can be rotated).
- Code: connectors `domain::outbound::retag` and
  `features::retag_bindings`, kernel `jobs::rekey` (doc only).
- Docs: `deploy/README.md`.
