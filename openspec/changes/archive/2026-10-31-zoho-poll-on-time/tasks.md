# Tasks

## 1. Don't miss the tick
- [x] 1.1 `schedule::TICK_SLACK` and `schedule::next_poll`, with unit tests (slack taken off, least wait kept whole, never before now, cadence across ticks)
- [x] 1.2 Mail streams (`sync_stream`) and calendar streams use `next_poll` for their next adaptive poll; leases, rate-limit waits and reauth are unchanged

## 2. Zoho mail polled at most a minute apart
- [x] 2.1 `ZohoMailConfig.poll_max_seconds` read leniently; accessor falls back to 60; warning at startup in `Deps::from_figment`
- [x] 2.2 The Zoho mail stream's interval is capped at it before it is stored and scheduled
- [x] 2.3 Config tests (default, valid, unusable values, production's poll bounds) and Zoho flow tests

## 3. Finish
- [x] 3.1 `deploy/.env.example` documents `CAMPBOOKS_CONNECTORS__ZOHO__POLL_MAX_SECONDS`
- [x] 3.2 Changelog entry file
- [x] 3.3 Change archived; baseline specs updated
