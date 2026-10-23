# 0020 — Machine-readable dates may assert

**Status:** Accepted · 2026-10-23 · amends [0018](0018-a-model-reads-email-content-facts-only-say-no.md)
· Tracking: notacamp/campbooks-cloud#42 · Change: `openspec/changes/add-schedule-module`

## Context
ADR 0018 says no code may decide what a message means from its subject,
body or attachment names, and that deterministic facts may only say "no".
The calendar (campbooks-cloud#42) needs the dates in mail: flights, hotel
stays, tickets, deliveries, invitations, bills. The owner asked for this to
cost as little as possible: Jev first, and at most one `complete` read per
email, only for mail Jev flagged and a member opened or that raised a need.

Much of that mail already carries its dates in a form built for machines:
- An invitation has a `text/calendar` part (iCalendar, RFC 5545) with typed
  start, end, time zone, `UID`, `SEQUENCE` and `METHOD`.
- Airlines, rail operators, hotels, ticket sellers, carriers and many billers
  embed schema.org markup (`FlightReservation`, `LodgingReservation`,
  `EventReservation`, `ParcelDelivery`, `Invoice`…) as JSON-LD or microdata
  for mail clients to read.

Reading these is not reading prose. The sender wrote typed fields for a
parser, and a standard parser reads them. A model asked to read the same
dates out of the email can only do worse, and it costs a call.

Markup is easy to forge, though. A phishing email could add a fake "flight
changed" reservation.

## Decision
1. **Data a sender structured for machines is not content in the sense of
   ADR 0018.** An iCalendar part, or schema.org markup in JSON-LD or
   microdata, MAY assert a dated item or an ask. It is read with a proper
   parser of that format, only its typed fields are used, and the prose
   around it is never read.
2. **Only from authenticated senders.** It applies only when the receiving
   provider's authentication results show the sender's domain passed DMARC
   aligned (read strictly, as `ai-decide-facts` specifies), and the decide
   pass did not accept `suspicious`. Otherwise the message is treated as
   having no markup.
3. **It wins over the model.** Items found this way are stored as the
   message's read (layer `structured`). Mail they cover is not read by the
   model for dates. A later model read gets them as items it already knows.
4. **A closed list of types.** iCalendar `VEVENT` and the schema.org
   reservation, delivery and invoice types named in the change. Any other
   type is ignored until a change adds it.
5. **Everything else in ADR 0018 stands.** No pattern, phrase list or
   regular expression looks for dates, amounts or intent in an email's
   words, in any language.

## Consequences
- Most bookings from large senders reach the calendar at no AI cost, even
  when nobody opens the email and it raises no need.
- An iCalendar parser and an HTML/JSON-LD reader become dependencies of
  `conversations`, under the usual supply-chain gates (`cargo deny`).
- Small senders that use no markup still need the model read, which runs
  only when a member opens the email or it raises a need.
- The facts source test of ADR 0018 is unchanged. The structured reader
  lives apart from `signals/facts.rs` and reads only parts and elements by
  media type and schema type.

## Alternatives considered
- **Leave everything to the model.** It costs one call per booking and is
  less exact than the typed fields.
- **Trust markup from any sender.** That lets forged events onto a calendar,
  and phishing would use it.
- **Use markup only to veto the model.** This throws away the most exact
  source there is, and the model call still happens.
