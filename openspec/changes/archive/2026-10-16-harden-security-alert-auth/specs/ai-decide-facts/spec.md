## ADDED Requirements

### Requirement: Authentication results are read strictly
The facts SHALL read the receiving provider's `Authentication-Results` as RFC
8601 says. A value longer than 8 KiB (8192 bytes) SHALL be unread and SHALL
count as authentication failed (a sender may not pad a header to hide a
failure); an absent or malformed one says nothing. The sender's domain SHALL count as authenticated only by exactly one
`dmarc` result, `pass`, whose `header.from` is the From domain; or by a
`dkim=pass` whose signing domains all have the From domain's registrable
domain, while no `dmarc` result in the header refuses it: only `pass`, `none`,
`bestguesspass`, `temperror` and `permerror` are not refusals, so `fail`,
`quarantine`, `reject`, `policy` and any non-standard result are. A From
domain that is itself a public suffix SHALL authenticate nothing. A
registrable domain SHALL be read from the Public Suffix List, ICANN and
private sections (compiled in, never fetched), so that two names under one
public suffix (`x.com.br` and `y.com.br`, `a.github.io` and `b.github.io`)
are never the same organisation; a name under a top-level domain the list
does not know SHALL be its own suffix, and a name the list cannot split SHALL
be kept whole. The RFC 2606 documentation domains (`example.com`,
`example.org`, `example.net`) SHALL stay suffixes so fixtures can stand for
many organisations. A taught `sender_domain` SHALL NOT name a public suffix
alone.

#### Scenario: A signature DMARC refused
- **WHEN** a message carries `dkim=pass header.d=studio.example` for a From of `studio.example` and a `dmarc` result of `fail`, `quarantine`, `reject`, `policy` or any other non-standard value, or two `dmarc` results of which one is `fail`
- **THEN** the hints say authentication failed

#### Scenario: A signature and no DMARC result
- **WHEN** a message carries `dkim=pass header.d=studio.example` for a From of `studio.example` and no `dmarc` result, a `dmarc` pass, or a `dmarc` of `none`, `bestguesspass`, `temperror` or `permerror`
- **THEN** the hints do not say authentication failed

#### Scenario: A padded results header
- **WHEN** the provider's `Authentication-Results` is longer than 8192 bytes
- **THEN** the hints say authentication failed, while one of exactly 8192 bytes is read as usual

#### Scenario: A signer under a shared suffix
- **WHEN** a message from `ana@y.com.br` is signed only by `x.com.br`, or one from `ana@b.github.io` only by `a.github.io`
- **THEN** the hints say authentication failed, while mail from `mail.y.com.br` signed by `y.com.br` is authenticated
