# Design

## A rule action read at open time

Every other taught action changes the conversation when mail arrives or when
a rule is applied. Showing images is about how a body is rendered, so the
engine's `evaluate` ignores `show_images` (arrival and runs do nothing for
it) and a separate `taught::shows_images(rules, facts, authenticated)` picks
the rule, most specific first, with the guards: authenticated, not
suspicious, no failed checks. `get_message_body` loads the inbox's acting
rules (which already honour paused, suspended authors and read-only
inboxes) only when one of them shows images.

`create_rule` skips the apply run for a rule that only shows images, and
`create_rule` / `update_rule` check that the inbox has inbound mail from the
sender it names (`rules::sender_seen`). The query is a runtime one, so the
offline query cache does not change.

## Authenticated, not merely "not failed"

The existing `sender_checks_failed` fact says nothing when there are no
results. Trust needs the opposite default, so `sender_authenticated` is a
positive test reusing the same alignment rules: no results, `none`, an
unreadable header or any DMARC failure is not a pass. A domain rule matches
subdomains of the domain, and the pass must be aligned with the From
domain itself, so "anyone at brightloop.example" never trusts mail signed by
someone else.

## Client

The smart-rule confirmation takes optional `choices` (the address, or anyone
at its domain) and, for a rule that shows images, says how many
conversations it opens up instead of what it would change, hides "apply to
the mail already here", and adds a line about sender checks and tracking
pixels. The free mail list moved to `lib/mail/free-mail.ts`, shared with
Scout's memory. Rule mutations refresh message bodies, so saving, undoing
or removing the rule shows or holds back the images at once.
