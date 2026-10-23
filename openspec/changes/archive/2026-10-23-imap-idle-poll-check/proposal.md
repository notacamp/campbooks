# Check the IDLE safety poll only while IDLE runs

## Why

The IMAP push change (#641) refused to start the server when
`CAMPBOOKS_CONNECTORS__IMAP__IDLE_POLL_SECONDS` (default 900) fell outside the
instance's poll bounds. That check ran even with IMAP and IDLE off. An instance
that narrows its poll bounds below 900 s then failed every boot, and Next prod
crash-looped on 2026-10-23 after the deploy of 4c6c0153.

## What changes

- The IDLE safety poll must still be positive. It is tied to the poll bounds
  only when both `CAMPBOOKS_CONNECTORS__IMAP__ENABLED` and
  `CAMPBOOKS_CONNECTORS__IMAP__IDLE_ENABLED` are on.
- A test starts the server with narrowed poll bounds and IDLE off. With IDLE on,
  the same config is refused, naming the setting.

## Impact

`crates/modules/connectors/src/config.rs` only. There is no behaviour change
while IDLE runs.
