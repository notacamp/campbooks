## ADDED Requirements

### Requirement: Sending only what the member was shown
The composer SHALL send, with the version it saved, the `content_token` of
the content on screen: the one of the draft it opened with, then of each
save's answer, never one from a list or a read it does not show. On 409
`content_changed` it SHALL read the draft again and show it as it is now
(its recipients, Cc and Bcc included, its subject and text), with "This
changed since you opened it. Check it and send again." in all four locales,
and SHALL NOT send it again on its own. The message composer SHALL show Cc
and Bcc whenever the draft has any, also ones that arrive after it opened.
The reply dialog SHALL show every recipient the reply goes to, SHALL save
exactly those with no Cc or Bcc, and SHALL send with the token of that save;
on 409 `content_changed` it SHALL say so and send nothing until the member
sends again.

#### Scenario: The draft changed since it was opened
- **WHEN** Sam presses Send and the server answers 409 `content_changed`
- **THEN** the composer shows the draft as the server holds it, a Bcc Sam never added included, with the notice, and nothing is sent until Sam presses Send again

#### Scenario: Opened before the upgrade
- **WHEN** Sam presses Send in a composer opened before the upgrade
- **THEN** the notice shows once, Sam's text is still there, and the next Send works
