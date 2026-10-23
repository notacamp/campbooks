# Proposal

## Why

campbooks-cloud#322 runs RabbitMQ for the hosted cloud and turns the relay
on. The cloud generates the broker users' passwords into a volume, the way
it does for the database's runtime login, and hands the server a file. The
relay only reads the password from `CAMPBOOKS_TRACKING__AMQP_URL`, so the
secret would have to sit in the environment, where `docker inspect` and the
process list show it.

Modules touched: the kernel (tracking config and the relay's connect). No
migration, endpoint, scope, event or new dependency.

## What Changes

- `CAMPBOOKS_TRACKING__AMQP_PASSWORD_FILE`: a file holding the broker
  password, mirroring `CAMPBOOKS_DATABASE_PASSWORD_FILE`. Its password (a
  trailing newline dropped) replaces any in `AMQP_URL`, so the URL can omit
  it. Blank counts as unset.
- With the sink `amqp`, a missing, unreadable or empty file stops the start.
  The message names the setting and the file's path, never the URL or the
  password. With another sink the file is not read.
- The relay connects with the resolved URL; it is never logged.

## Impact

- Specs: `tracked-events` (new requirement: the broker password can come from
  a file).
- Docs: `docs/tracking.md` settings, `deploy/README.md` → Tracked events,
  `deploy/.env.example`.
