## ADDED Requirements

### Requirement: A Microsoft mail connection goes to the mail tenant
A connect or reconnect round trip with Microsoft SHALL go to the tenant set by
`CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT` (`common` by default): its
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

#### Scenario: Scopes answered with the Graph prefix
- **WHEN** Microsoft's token answer grants `https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/User.Read`
- **THEN** the mailbox counts as granted
