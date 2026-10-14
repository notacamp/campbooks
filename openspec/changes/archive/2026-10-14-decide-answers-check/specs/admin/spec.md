## MODIFIED Requirements

### Requirement: System panel
The system module SHALL expose `GET /admin/v1/system/overview` (permission
`system.read`) returning the version and commit, the latest applied migration
and the migration count, the process start time, the database's health, a
summary of the effective configuration that contains no secrets (signup
policy, configured sign-in providers, blob store kind, configured mail
providers), and per AI role whether it is configured, its model, its base URL
reduced to scheme and host, and whether it answered a probe within 3 seconds
(probe results cached for 60 seconds). For the `decide` role, when it may
receive mail, it SHALL also return its answers check: `pending`, `passed`,
`failed` or `unavailable`, with the right and total counts and when it ran.
The System panel SHALL show a failed or unavailable check as a problem and
say that the model's answers are not used.

#### Scenario: No secrets
- **WHEN** an AI role's base URL contains credentials or the configuration holds API keys
- **THEN** the overview shows only the scheme and host and never any key or password

#### Scenario: A decide model failing its answers check
- **WHEN** the decide model got 8 of 33 answers of its check right
- **THEN** the System panel shows the decide role as a problem: "Answers fail the check", got only 8 of 33 obvious test answers right, its answers are not used and the rules sort mail alone
