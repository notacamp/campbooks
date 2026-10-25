## ADDED Requirements

### Requirement: A Microsoft mail connection goes to the mail tenant
A connect or reconnect round trip with Microsoft SHALL go to the tenant set by
`CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` (unset: sign-in's own): its
authorization request, code exchange and ID-token validation (the issuer
computed from the token's own `tid`, as for sign-in), with the same state,
nonce, PKCE and browser binding. A sign-in and a link SHALL keep sign-in's
tenant (`CAMPBOOKS_MICROSOFT__TENANT`). An invalid mail tenant SHALL not stop
the identity module (the connectors module refuses it while Microsoft mail is
on). A connect that would add a way in SHALL add one only when sign-in's
tenant admits the account; otherwise it SHALL connect the mailbox as a shared
one is connected (no way in). Whether the round trip granted the mailbox SHALL
compare Microsoft's granted scopes without the `https://graph.microsoft.com/`
prefix and ignoring case.

#### Scenario: A personal account on an instance whose sign-in is for organizations
- **WHEN** sign-in's tenant is `organizations`, the mail tenant `common`, and a member connects an Outlook.com mailbox
- **THEN** the browser is sent to the `common` authorization endpoint, the mailbox is connected, and no way in is added

#### Scenario: The mail tenant unset
- **WHEN** sign-in's tenant is one tenant id and `CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` is unset
- **THEN** a connect goes to that tenant's authorization endpoint

#### Scenario: Scopes answered with the Graph prefix
- **WHEN** Microsoft's token answer grants `https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/User.Read`
- **THEN** the mailbox counts as granted

### Requirement: Signing in with Microsoft asks for mail only for a new account
A sign-in with Microsoft SHALL ask for no mailbox scope and no offline access
when the account turns out to be known, as for Zoho: Microsoft cannot revoke
one app's refresh token, and a work tenant may need an administrator's consent
for mail. Only when the account is new, and Microsoft mail is offered, SHALL a
second round trip ask for the mailbox (with consent), so the first mailbox can
come with sign-up. A connect and a reconnect SHALL always ask for it.

#### Scenario: A known account signs in
- **WHEN** a person whose Microsoft account is already a way in signs in with Microsoft on an instance that offers Microsoft mail
- **THEN** the authorization request carries no mail scope and no `offline_access`, and no grant is held

#### Scenario: A new account signs up
- **WHEN** a person signs up with a Microsoft account Campbooks doesn't know, on an instance that offers Microsoft mail
- **THEN** a second round trip asks for the mailbox with consent, and its grant is held for the new account
