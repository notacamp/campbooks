## ADDED Requirements

### Requirement: The broker password can come from a file
With `CAMPBOOKS_TRACKING__AMQP_PASSWORD_FILE` set, the relay SHALL connect
with the password held in that file (a trailing newline dropped) in place of
any password in `CAMPBOOKS_TRACKING__AMQP_URL`. With the sink `amqp`, a
missing, unreadable or empty file SHALL stop the server at start with a
message that names the setting and never shows the URL or the password.

#### Scenario: A URL without its password
- **WHEN** the URL names the user `campbooks-relay` and no password, and the file holds the user's password
- **THEN** the relay signs in to the broker and publishes

#### Scenario: A wrong password in the file
- **WHEN** the URL carries the right password and the file a wrong one
- **THEN** the broker refuses the relay: the file's password is the one used

#### Scenario: The file is missing
- **WHEN** the sink is `amqp` and the file does not exist
- **THEN** the server does not start, and its message names `CAMPBOOKS_TRACKING__AMQP_PASSWORD_FILE` and not the URL
