## Context
The signup policy applies to creating a workspace and, on `invite_only`, to
creating an account. Nothing restricts who may sign in.

## Decisions
- **Name and home.** `CAMPBOOKS_SIGNUP__ALLOWED` sits beside the existing
  `CAMPBOOKS_SIGNUP__POLICY` and `__CODES`, parsed by the kernel
  (`config::AllowList`) and validated by `Config::check`. It is not secret.
  Logs and errors never print the entries; a parse error names the entry's
  position.
- **Which address.** A provider sign-in or link is judged on the address the
  provider verified in that round trip. A later door (passkey, token
  exchange) is judged on the verified address recorded when each way in was
  linked (`provider_identities.email_at_link`): an account qualifies if any
  of its ways in does. A passkey proves no address, so a passkey-only
  account does not qualify and a passkey cannot create an account.
- **Order.** The callback asks the allow-list before it looks for an
  account, so a refusal is the same for known and unknown people.
- **Token exchange is the choke point.** Both `authorization_code` and
  `refresh_token` end in one place that already refuses suspended accounts;
  the allow-list check sits next to it and answers `invalid_grant` with the
  transaction rolled back. The web BFF, which only holds a refresh token,
  therefore loses a removed person at its next refresh. Client-credentials
  tokens belong to a client, not a person, and are unchanged.
- **Scopes and data.** No new endpoint, scope or schema; the identity
  schema is only read.

## Risks
- A mistyped list locks everyone out: a malformed entry stops startup, but a
  well-formed wrong one does not. The operator fixes the variable and
  restarts.
- The recorded address can go stale if the person changes it at the
  provider; they are judged on what was verified when they linked.
