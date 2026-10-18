## ADDED Requirements

### Requirement: The instance secret key can be rotated
The server SHALL accept a previous instance secret key beside the current one
(`CAMPBOOKS_SECRET_KEY_PREVIOUS_FILE` or `CAMPBOOKS_SECRET_KEY_PREVIOUS`).
Everything SHALL be sealed, signed and derived with the current key. The
previous key SHALL only open and verify what was sealed or signed before:
- web sessions' and sign-ins' sealed values;
- connected accounts' credentials;
- the sign-in signing keys and held grants;
- signed and sealed opaque values (cursors, resume tokens);
- the binding tags of sends and changes at mail providers.

What the previous key opened SHALL be sealed again with the current key as it
is used:
- the signing keys when they are loaded;
- a connection's access token when it is used, and its refresh token at its
  next refresh;
- a web session's refresh token at its next refresh.

The server SHALL refuse to start, naming the variable, when a previous key is
set without a current one, is the same key, or is not 32 bytes of base64. No
key SHALL appear in a log, an error or a `Debug` output. Without the previous
key, what only it sealed SHALL fail closed, as for any wrong key.

#### Scenario: Old values open and are sealed again
- **WHEN** the key is rotated with the old one as the previous key, and a connection's access token is used, its token is refreshed, and the signing keys are loaded
- **THEN** each opens, and is stored sealed with the new key, which alone opens it afterwards

#### Scenario: The previous key removed
- **WHEN** the previous key is removed after everything was sealed again
- **THEN** the new key alone opens everything, and the old key alone opens nothing

#### Scenario: A wrong key
- **WHEN** neither the current nor the previous key sealed a value
- **THEN** it does not open: a connection needs connecting again, and the server refuses to start on signing keys it cannot open

#### Scenario: A misconfigured previous key
- **WHEN** `CAMPBOOKS_SECRET_KEY_PREVIOUS` is set without `CAMPBOOKS_SECRET_KEY`, equal to it, or not valid base64
- **THEN** the server refuses to start with an error naming the variable and showing no key
