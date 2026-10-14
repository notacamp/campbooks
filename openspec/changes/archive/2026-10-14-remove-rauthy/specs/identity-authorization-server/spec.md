## ADDED Requirements

### Requirement: The shipped self-host stack runs no separate identity provider
The shipped `deploy/compose.yaml` SHALL run no identity-provider service and SHALL NOT set `CAMPBOOKS_AUTH__ISSUER`, so a default install signs people in through the issuer in the server. Provider client secrets SHALL be files under `deploy/secrets/`, mounted read-only into the server.

#### Scenario: A fresh stack
- **WHEN** a self-hoster runs `docker compose up -d` with the shipped files and only the required settings
- **THEN** the only services are the server, Postgres and their one-shot helpers, and the server serves `/.well-known/openid-configuration` on its own origin
