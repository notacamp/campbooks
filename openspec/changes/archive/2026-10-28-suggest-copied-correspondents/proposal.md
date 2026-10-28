# Suggest people copied on received mail in To, Cc and Bcc

Modules: `conversations` (the `conversations.list_correspondents` read).
Tracking: https://github.com/notacamp/campbooks-cloud/issues/395. Follow-up
of campbooks-cloud#304. No new module, no migration, no client change.

## Why

The composer suggests the people you exchange mail with, but only the senders
of the mail you receive and the people you write to. Someone copied on a
thread you are part of (Leo, in Cc of Maya's email) is not suggested until you
have written to them once, by typing their address from memory. Everyone on
an email you received is someone you can be expected to write to.

## What changes

- **People copied on received mail count.** `list_correspondents` also reads
  the To and Cc recipients of the inbound mail it already reads (the newest
  5,000 inbound messages of the last 365 days, not in spam or the trash),
  apart from the inboxes' own addresses. Same bounds, same indexes.
- **Ranked after the people you actually exchange mail with.** By `wrote`,
  then `received`, then how many received messages copied them, then by
  address. The response is unchanged: no new field.
- **Safe to write to, from facts.** Someone only copied (never written to,
  never wrote in) is suggested only when they are not a no-reply address,
  none of their own mail is flagged, and the newest message that copied them
  is from a sender who may be written to: that sender's checks pass on that
  message (`may_write_to`) and none of the sender's mail is reported, under
  review or found suspicious and not said safe. A phishing email cannot plant
  an address in the composer.

## Non-goals

- A contacts table or the provider's address book.
- Suggesting Bcc recipients of received mail (providers don't expose them).
- Any rule that reads what an email says (ADR 0018).

## Impact

- `conversations`: the `list_correspondents` query and its Rust check;
  `flagged_sender_sql!` takes the address column so it can judge a copied
  person's sender too (`list_senders` unchanged). HTTP tests.
- OpenAPI: the ranking line of `CorrespondentList.correspondents`, and the
  generated client types.
