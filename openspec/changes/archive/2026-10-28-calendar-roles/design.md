# Design

## Storing the role

Alternatives:

1. **A `role` column (`mine` | `following` | `off`) replacing `syncing`.**
   This is the cleanest shape, but every reader of `syncing` changes (sync
   requests, the change feed, `calendar_sync_changed`, invitations, free time,
   event listing). That is a wide diff with no behaviour gain.
2. **A `following boolean` beside `syncing` (chosen).** The role is derived:
   Off = `!syncing`, Following = `syncing && following`, Mine = `syncing &&
   !following`. Turning a calendar on and off is unchanged, so connectors never
   learn about roles. An Off calendar keeps whether it would come back Mine or
   Following. Only free time, holds and invitations read `following`.

A `CHECK (source = 'provider' OR NOT following)` keeps the Campbooks calendar
Mine.

## The ownership fact

`owned boolean NULL` on `schedule.calendars` is the provider's word, kept
apart from the member's choice so the UI can say "shared with you" and a
default can be recomputed. `NULL` means unknown: the calendar was made before
the list carried the fact, or by a provider that does not say.

- Google: `accessRole == "owner"`. The values `writer`, `reader` and
  `freeBusyReader` are not owned. Without an `accessRole` the fact is unknown.
- Zoho: `privilege == "owner"` is owned, any other value is not, and without
  one the fact is unknown.

## Defaults

- **New calendar:** `following = owned is false`. Unknown counts as Mine, as
  today. `syncing = primary`, as today.
- **Backfill in the migration:** `following = true` for provider calendars that
  cannot be written. A calendar you cannot write is never yours: a holidays
  calendar, or a colleague's calendar shared read-only. Every other calendar
  stays as it is, so nothing that counts today stops counting without the
  member choosing it.
- **Repeated grant:** sets `owned` where it is `NULL` and leaves `following`
  alone. The member's choice is never overwritten. The proposal names this
  exception to "a repeated grant changes nothing".

## Invitations

`invitations::standing` returns `InvitationClosed::Hidden` for a Following
calendar, exactly as it does for one that does not sync. `update` already
calls `announce_calendar` when `syncing` changes; it also calls it when
`following` changes, so the Decide need leaves Home (or comes back) in the
same transaction.

## Holds

`holds` refuses a Following calendar with the reason `calendar_followed`
(400, like `calendar_read_only`), worded in the server's four languages.
When no calendar is named, the Campbooks calendar is used, as today. The
client's picker lists only writable Mine calendars.

## The PATCH

`role: "mine" | "following" | "off"` sets both columns, and `syncing` stays
accepted. When a patch carries both, they must agree, or the answer is 400
`invalid_calendar`. Giving the Campbooks calendar any role but `mine` is
also 400 `invalid_calendar`. Going from Off to Mine or Following asks
connectors to read the calendar from the start (`calendar_sync_changed`), as
turning it on does today; moving between Mine and Following publishes no
sync change. The tracked events `CalendarEnabled`/`CalendarDisabled` keep
their meaning (on/off). A role change between Mine and Following is tracked
as a new `CalendarRoleChanged` with the role. It carries no name.
