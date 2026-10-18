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
- a web session's refresh and ID tokens at its next refresh.

What the previous key signed SHALL be signed again with the current key in
the background. While a previous key is set, a job SHALL run in every
workspace with a change feed, at most every 15 minutes. It SHALL re-tag
every taught rule (at its generation), rule application, running rule run,
change request and scheduled or stored send whose MAC verifies under the
previous key only. Each update SHALL be a compare-and-set on the old MAC, so
a row signed again meanwhile is left as it is, and a row that verifies under
neither key SHALL NOT be re-tagged. Each run SHALL log, per workspace,
either that nothing is bound under the previous key any more or how many
rows are left for the next run.

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

#### Scenario: Rules and their changes signed again
- **WHEN** a rule moved mail under the old key, the key is rotated, the re-tag runs and the previous key is removed
- **THEN** the rule still acts on new mail, its changes are still put back, and a second re-tag finds nothing to do

#### Scenario: A wrong key
- **WHEN** neither the current nor the previous key sealed a value
- **THEN** it does not open: a connection needs connecting again, and the server refuses to start on signing keys it cannot open

#### Scenario: A misconfigured previous key
- **WHEN** `CAMPBOOKS_SECRET_KEY_PREVIOUS` is set without `CAMPBOOKS_SECRET_KEY`, equal to it, or not valid base64
- **THEN** the server refuses to start with an error naming the variable and showing no key
