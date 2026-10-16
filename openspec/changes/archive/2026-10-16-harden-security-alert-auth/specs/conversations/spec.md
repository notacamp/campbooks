## MODIFIED Requirements

### Requirement: Security alerts are a to-do
The email signals SHALL answer `asks_action` true with the reason
`signals.security_alert`, and `stream` `updates`, for a message whose subject
names a security problem (a security alert or advisory, a vulnerability, in
English, European Portuguese, Spanish and French; "action required" alone is
not one) only when all of these hold: the sender is a service (list mail or
an automated address); its domain is plain ASCII (no internationalised
label), is not itself a public suffix, and its display name names that
domain; the receiving provider's own authentication results, read as RFC 8601
says (comments and quoted strings are not results; a value over 8 KiB is no
results), hold exactly one `dmarc` result, `pass`, whose own `header.from` is
the From domain, or a `dkim=pass` whose own signing domain has the From
domain's registrable domain while no `dmarc` result in the header says
anything but `pass`; the message has no phishing tell and carries no
marketing words. A registrable domain SHALL be read from the Public Suffix
List (ICANN and private sections), so that two names under one public suffix
(`x.com.br` and `y.com.br`, `a.github.io` and `b.github.io`) are never the
same domain; a name under a top-level domain the list does not know SHALL be
its own suffix. It SHALL take precedence over the list-mail "no action"
answer. A sure `do` need from a security alert SHALL require an authenticated
sender on the listed code hosts and package registries (`github.com`,
`gitlab.com`, `bitbucket.org`, `npmjs.com`, `pypi.org`, `rubygems.org`,
`crates.io`, `snyk.io`) whose whole display name is the platform's own (letters
and digits compared: `GitHub`, `npm` for `npmjs.com`, `crates.io`), not a name
that merely holds it as a word; for any other authenticated sender the answer
SHALL stay below every confidence the policy acts from, so it raises at most
a question. Anything that offers a message's own links as the action on its
need SHALL offer them only for a message whose sender is authenticated this
way and that is not suspicious.

#### Scenario: A dependency vulnerability
- **WHEN** "GitHub" `<notifications@github.com>` sends list mail titled "[example-org/demo-shop] High severity vulnerability in tiny-template" with Review, Dismiss and Upgrade links, the receiving provider reports `dkim=pass` and `dmarc=pass` for `github.com`, and the model also answers `asks_action` true with 0.875
- **THEN** the conversation is sorted to Updates and has a `sure` `do` need with basis `rules`

#### Scenario: The same alert, not authenticated
- **WHEN** the same alert arrives with no authentication results from the receiving provider, or with `dmarc=fail`
- **THEN** no `do` need is raised

#### Scenario: A signature DMARC refused
- **WHEN** the same alert arrives with `dkim=pass header.d=github.com` and a `dmarc` result of `fail`, `none`, `quarantine`, `temperror`, `permerror` or `policy`, or with two `dmarc` results of which one is `fail`
- **THEN** no `do` need is raised

#### Scenario: A signature and no DMARC result
- **WHEN** the same alert arrives with `dkim=pass header.d=github.com` and no `dmarc` result at all
- **THEN** a `sure` `do` need is raised

#### Scenario: A padded results header
- **WHEN** the provider's `Authentication-Results` is longer than 8192 bytes
- **THEN** it is read as no results, and one of exactly 8192 bytes is read as usual

#### Scenario: A signer under a shared suffix
- **WHEN** a message from `noreply@y.com.br` is signed only by `x.com.br`, or one from `b.github.io` only by `a.github.io`
- **THEN** the signature does not authenticate the sender, while one from `mail.y.com.br` signed by `y.com.br` does

#### Scenario: A person named like the platform
- **WHEN** a notification from `notifications@github.com` shows the display name "GitHub Smith" and is otherwise an authenticated, security-titled alert
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: A crafted envelope sender
- **WHEN** a message from "Crates" `<alerts@crates.io>` fails DMARC, and its envelope sender `"a; dmarc=pass header.from=crates.io x"@evil.example` is echoed in the provider's results
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: A look-alike that signs its own mail
- **WHEN** "GitHub Security" `<alerts@githubsecurity.example>`, with `dkim=pass` and `dmarc=pass` for its own domain, sends "Security alert: new sign-in to your account"
- **THEN** no `sure` need is raised

#### Scenario: An internationalised look-alike
- **WHEN** the sender's domain spells "github" with a Cyrillic letter, or as `xn--` punycode
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: Action required alone
- **WHEN** an authenticated GitHub notification's subject says "Action required: review your settings" and names no security problem
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: A comment on a watched issue
- **WHEN** the same code host sends a notification of a comment on an issue the member watches
- **THEN** no need is raised

#### Scenario: Marketing that says action required
- **WHEN** a newsletter's subject is "Action required: claim your discount"
- **THEN** `asks_action` is false and the stream is Newsletters

### Requirement: Spam and trash need nothing
A message in spam or the trash SHALL never raise a need: a conversation's
need SHALL come from its newest inbound message that is in neither, and a
need whose own message moves to spam or the trash SHALL be cleared as
`trashed`. Working out a need SHALL hold the messages it can come from
against a concurrent move to spam or the trash (taking them before the need's
own row), so that no open need is stored for a message after its move
committed. A message that leaves spam or the trash SHALL raise its need again
as any message does; a member's Done or Not needed SHALL still hold, and a
member's confirmation SHALL still make the reopened need theirs (basis
`member`).

#### Scenario: The lease goes to the trash
- **WHEN** the message Pedro's `do` need comes from is moved to the trash, while his later message stays
- **THEN** the need is cleared with cause `trashed`

#### Scenario: A question in spam
- **WHEN** a question lands in spam inside a conversation whose need comes from an earlier message
- **THEN** the need still comes from the earlier message

#### Scenario: Not spam after all
- **WHEN** the message Marta's `reply` need comes from is moved to spam and later back to the inbox
- **THEN** the need is cleared as `trashed`, then open again from the same message

#### Scenario: A trash round trip
- **WHEN** a confirmed need's message and a Done need's message are trashed and then restored
- **THEN** the confirmed need is open with basis `member`, and the Done one stays cleared as `done`

#### Scenario: A move and a refresh at once
- **WHEN** a decide pass refreshes a conversation while a provider move of its message to spam is not yet committed
- **THEN** the refresh waits for the move and stores no need for that message
