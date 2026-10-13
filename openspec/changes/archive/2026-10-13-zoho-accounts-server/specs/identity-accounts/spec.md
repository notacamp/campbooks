## ADDED Requirements

### Requirement: Zoho accounts server for the sign-in request
The authorization request to Zoho SHALL go to the accounts server named by `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER`, default `https://accounts.zoho.com`. The setting SHALL be exactly one of Zoho's accounts servers; any other value (another host, `http`, a path) SHALL stop startup with an error naming the setting. The callback's `accounts-server` handling is unchanged.

#### Scenario: Default
- **WHEN** the setting is unset
- **THEN** the authorization URL starts with `https://accounts.zoho.com/oauth/v2/auth`

#### Scenario: A European app
- **WHEN** the setting is `https://accounts.zoho.eu`
- **THEN** the authorization URL starts with `https://accounts.zoho.eu/oauth/v2/auth`

#### Scenario: A value that is not a Zoho accounts server
- **WHEN** the setting is `https://accounts.example.com`, `http://accounts.zoho.eu` or `https://accounts.zoho.eu/oauth`
- **THEN** startup fails with an error naming `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER`
