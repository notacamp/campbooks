## ADDED Requirements

### Requirement: Calendar settings say how adding calendar access came back
Settings → Calendar SHALL read `?connected=` and `?link_error=` when the member
returns from the provider, once, and clear them from the address. On success it
SHALL say calendar access was added, naming the mailbox's address. On
`account_mismatch` it SHALL say the provider answered as another account and
name the mailbox's address to switch to at that provider; when it cannot tell
which mailbox, it SHALL say so without an address. A cancelled return and any
other refusal SHALL each have their own short sentence. It SHALL be in en,
pt-PT, es-ES and fr, and fit 375px.

#### Scenario: Zoho answered as another account
- **WHEN** the member returns to Settings → Calendar with `link_error=account_mismatch` after adding access to their Zoho mailbox
- **THEN** the page says "Zoho answered as another account. Switch to that mailbox's address at Zoho, then try again." and the query is cleared

#### Scenario: Calendar access was added
- **WHEN** the member returns with `connected=<connection>`
- **THEN** the page says calendar access was added for that mailbox's address
