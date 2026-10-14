# Tasks

- [x] `raise`: a model suggestion is not a question when an acted answer to the same question says no; the rules' acted answer wins
- [x] `refresh`: withdraw an open need its own message no longer raises (cleared `superseded`), unless a member confirmed it
- [x] `signals.security_alert`: security words in four languages (no bare "action required"), service sender with a plain ASCII domain its name names, authenticated by the receiving provider, no phishing tell, no marketing; `asks_action` true, `stream` Updates; acts only for listed platforms
- [x] Connectors: keep the receiving provider's own topmost `Authentication-Results` in raw order (not UTF-8: none); Gmail only, Zoho none until verified (campbooks-cloud#132); contract field
- [x] RFC 8601 tokeniser: comments, quoted strings, per-result properties; one `dmarc` result with its own `header.from`
- [x] Per-platform display names; a positive test per listed platform
- [x] Spam and trash: no need from such a message; a need whose message moves there is cleared
- [x] Fixtures: a security alert and a comment notification from a code host; contract test
- [x] Tests: every bypass of the security review (unauthenticated, failing DMARC, look-alikes with their own DKIM, Cyrillic and punycode domains, bare "action required") is no sure Do; the production shape raises nothing; a model-only yes is still a question; an acted rules yes is a sure Do; the full decide pass over both fixtures with a model stub; withdrawal is announced once
- [x] Reader copy for the new reason in en, pt-PT, es-ES, fr
- [x] Offline eval unchanged (`cargo xtask eval decide --offline --check`)
- [x] CHANGELOG updated, gates green
