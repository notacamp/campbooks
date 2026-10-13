# upstream-sign-in Specification

## Purpose
How a self-hosted Campbooks lets people sign in with Google, Microsoft or
Zoho through its identity provider, configured from settings (ADR 0004).

## Requirements

### Requirement: Sign-in providers from settings
`idp-provision` SHALL register a sign-in provider in the identity provider
when, and only when, its client ID is set (`CAMPBOOKS_IDP_GOOGLE_CLIENT_ID`,
`CAMPBOOKS_IDP_MICROSOFT_CLIENT_ID`, `CAMPBOOKS_IDP_ZOHO_CLIENT_ID`), reading
its client secret from a file (`CAMPBOOKS_IDP_<PROVIDER>_CLIENT_SECRET_FILE`)
and never from an environment variable. Each provider SHALL request the
scopes `openid email profile`, use PKCE (S256) and never link an existing
account automatically. Microsoft SHALL use the tenant
`CAMPBOOKS_IDP_MICROSOFT_TENANT` (default `common`) and Zoho the accounts
server `CAMPBOOKS_IDP_ZOHO_ISSUER` (default `https://accounts.zoho.eu`). When
no client ID is set, `idp-provision` SHALL make no provider call at all. A
client ID without a readable, non-empty secret file SHALL fail the run with a
message naming the file.

#### Scenario: Nothing configured
- **WHEN** no provider client ID is set
- **THEN** `idp-provision` succeeds without calling the providers API, and the login page offers passkey and password only

#### Scenario: Google configured
- **WHEN** `CAMPBOOKS_IDP_GOOGLE_CLIENT_ID` is set and its secret file is readable
- **THEN** a provider named `Google` of type `google` with issuer `https://accounts.google.com`, PKCE, `client_secret_post` and `auto_link` off exists after the run

#### Scenario: Secret file missing
- **WHEN** a client ID is set but its secret file is missing or empty
- **THEN** the run fails naming the file, before any provider call

### Requirement: Provisioning is idempotent and keeps admin edits
`idp-provision` SHALL create each configured provider once and record it. A
later run with the same settings (client ID, secret, issuer, new-account
rule) SHALL NOT change the provider, so edits made in the identity provider's
admin UI survive restarts and upgrades. When those settings change, the next
run SHALL replace the provider's configuration with the configured one and
say so. A provider with the same issuer that exists without a record SHALL
be configured and recorded instead of duplicated. A provider whose client ID
is no longer set SHALL stay registered, and the run SHALL say how to remove
it.

#### Scenario: Second run
- **WHEN** `idp-provision` runs again with unchanged settings
- **THEN** it makes no create or update call for that provider

#### Scenario: Secret rotated
- **WHEN** the secret file's content changes
- **THEN** the next run updates the provider with the new secret and reports that its configuration was re-applied

#### Scenario: Existing provider adopted
- **WHEN** the identity provider already has a provider with the configured issuer and no record exists
- **THEN** that provider is updated and recorded, and no second one is created

### Requirement: New accounts follow the signup policy
A provider SHALL create an identity-provider account for a person who has
none when `CAMPBOOKS_SIGNUP__POLICY` is `open`, `code` or `approval`, and
SHALL NOT when it is `invite_only` (only existing accounts may then use the
provider). `CAMPBOOKS_IDP_PROVIDER_SIGNUP` SHALL override this: `open` always
allows, `closed` never allows, `auto` (default) follows the policy. An
unknown value SHALL fail the run.

#### Scenario: Invite-only instance
- **WHEN** `CAMPBOOKS_SIGNUP__POLICY=invite_only` and `CAMPBOOKS_IDP_PROVIDER_SIGNUP` is unset
- **THEN** every provisioned provider has `auto_onboarding` off

#### Scenario: Explicit override
- **WHEN** `CAMPBOOKS_SIGNUP__POLICY=invite_only` and `CAMPBOOKS_IDP_PROVIDER_SIGNUP=open`
- **THEN** every provisioned provider has `auto_onboarding` on

### Requirement: Provider secrets never reach logs
`idp-provision` SHALL NOT write a provider's client secret to its output, its
error messages or a process's command line, including when the identity
provider's response echoes it. When the identity provider refuses the
providers API because its API key lacks the right (installs from before this
change), the run SHALL fail after provisioning scopes and clients, with
instructions to grant the right in the admin UI.

#### Scenario: Error response echoes the secret
- **WHEN** creating a provider fails and the response body contains the secret
- **THEN** the run fails and its output does not contain the secret

#### Scenario: API key without the providers right
- **WHEN** a provider is configured and the providers API answers 403
- **THEN** scopes and clients are provisioned, the run fails, and its message names the admin UI's API Keys page
