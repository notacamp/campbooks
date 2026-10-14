## REMOVED Requirements

### Requirement: Sign-in providers from settings
**Reason**: Rauthy is gone; `idp-provision` no longer exists. Campbooks' own sign-in offers Google, Microsoft and Zoho from `CAMPBOOKS_<PROVIDER>__*` settings (identity-authorization-server spec).
**Migration**: Move `CAMPBOOKS_IDP_<PROVIDER>_*` settings to `CAMPBOOKS_<PROVIDER>__CLIENT_ID` and `…__CLIENT_SECRET_FILE`; register `${CAMPBOOKS_PUBLIC_URL}/auth/callback/<provider>` with the provider.

### Requirement: Provisioning is idempotent and keeps admin edits
**Reason**: There is no provisioning step and no external admin UI to edit.
**Migration**: None.

### Requirement: New accounts follow the signup policy
**Reason**: Covered by the workspaces signup policy and the built-in issuer's account creation; the `CAMPBOOKS_IDP_PROVIDER_SIGNUP` override no longer exists.
**Migration**: Use `CAMPBOOKS_SIGNUP__POLICY`.

### Requirement: Provider secrets never reach logs
**Reason**: The provisioning script that handled these secrets is removed; provider client secrets are read from files by the server, which never logs secrets (AGENTS.md).
**Migration**: None.
