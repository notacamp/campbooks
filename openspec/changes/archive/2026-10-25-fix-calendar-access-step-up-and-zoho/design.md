# Design

## Decisions

### 1. The server says what a connection could still be given

`ConnectionView.offers: Capability[]`, computed by `Deps::offers(provider,
held)` where the rest of the module already decides what is offered
(`config.zoho_calendar` together with a configured Zoho client, as the sync
and the consent check do). The client reads it instead of naming providers,
so turning the Zoho adapter on needs no client change. Rejected: a boolean
`calendar_available` (does not extend to the next capability), and a
providers list in the ways-in answer (it is per connection, since a
connection that already holds calendar offers nothing).

### 2. The reconnect names what to add

`identity.start_link` gains `capabilities` (default none). Only `calendar` is
accepted (400 `link_incomplete` otherwise). The ticket's capabilities are the
union of the request's, the consent door's and, for a reconnect, those the
connection holds. Zoho's scopes are still added only while its flag is on, so
a request cannot bypass the flag. Rejected: calling
`connectors.start_connection` from the client: the client's `connectors`
module would need to reach `identity`'s step-up handling a second way.

### 3. The refusal is explained where it happens

`ConsentRefusal` is already typed. Settings → Calendar renders each value:
`step_up` as a banner with "Sign in again" (`consent.signInAgain`, which
returns to the current page, so to Settings → Calendar); the others as a short
sentence. No new state.

## Data, events, scopes

No tables, no events. `start_link` keeps `identity:write`; the connection
reads keep `connectors:read`. No AI. Tracking: no change (no operation, job or
handler added).
