## Authentication gate

`taught::Facts` gains `authenticated` (the receiving provider's results
authenticate the From domain: `signals::facts::sender_authenticated`).
`evaluate` drops the `highlight` slot unless `authenticated` and none of
`suspicious`, `auth_failed`, `impersonated` holds: the same gate as
`shows_images`, applied once in the engine so arrival, existing-mail runs and
the preview all agree. No results at all is not a pass.

## Only the newest on existing mail

An apply run already counts the matches in a first pass. That pass now also
finds the newest unread conversation the rule may raise (`can_be_looked_at`:
unread, not suspicious, authenticated), carried in the job payload
(`RunBatch.newest`, serde default: no migration). `rules::apply` takes whether
to raise the highlight for the conversation, true only for that one, so the
other matches get whatever else the rule does and no Home item. The preview
counts that one conversation for the highlight, matching the run. Arrival
(`apply_changes`) is untouched: one item per new message.

## Not counted as "need you"

The hero and the Home badge count items whose reason is not `worth_a_look`. The
group's own count stays. With only Worth a look items, the title is "Nothing
needs you" and the subline says how many emails are worth a look, so nothing is
hidden and nothing claims to be asked of the member.

## Shared inboxes

Needs belong to the conversation, so a Worth a look item exists for everyone who
can read the inbox, and opening the message clears it for all. This is the
existing behaviour, now specified; the confirmation says it when the goal is
Highlight.

## Shortcuts

Home's item keys are R, E, S, H, X, 1 to 3, J/K, arrows and Shift+E; the inbox
adds P, U, M, F, V, #, !, Shift+R, Shift+I, Shift+A. Scout noticed takes
Shift+F (its own verb), Shift+O (Something else), Shift+L (Later: Not now) and
Shift+D (Don't suggest), all free in every scope, live only while no
confirmation is open.
