## 1. The sign-in on the token

- [x] 1.1 `fam` on access tokens: the family a redemption creates, kept across rotation
- [x] 1.2 The verifier reads `sid` and `fam` of the in-process issuer's person tokens (`Principal::sign_in`)

## 2. The check

- [x] 2.1 `TokenRevocation::standing` takes the principal; `TokenStanding::sign_in_ended`
- [x] 2.2 One statement: account, ended session, revoked family, consent revoked since issue
- [x] 2.3 Index `identity.consents (account_id, client_id)`

## 3. Tests

- [x] 3.1 One session signed out, one refresh token revoked: that sign-in stops, the other works
- [x] 3.2 A way in removed, an app's consent revoked: those tokens stop at once, the person's own work
- [x] 3.3 The adapter: each kind ends its sign-in, a consent revoked before and granted again does not
