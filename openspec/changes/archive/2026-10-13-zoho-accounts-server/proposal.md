## Why
Sign-in with Zoho always sent people to `https://accounts.zoho.com/oauth/v2/auth`. A Zoho OAuth client registered in a single data centre (for example the EU console, used through `accounts.zoho.eu`) without Multi-DC is rejected there as an invalid client, so such an instance cannot offer Zoho sign-in.

## What Changes
- New optional setting `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER`, default `https://accounts.zoho.com`, used for the authorization URL. It must be exactly one of Zoho's accounts servers; anything else is refused at startup, naming the setting.
- Documented in `deploy/.env.example`, `deploy/README.md` and the changelog.

## Non-goals
- No change to the callback: its `accounts-server` is still checked against the allowlist, the code is still exchanged at the server Zoho names, and the ID token's issuer is still checked per data centre.
- No data-centre discovery, no per-user data centre, no Zoho mailbox adapter.

## Impact
Identity module configuration only. No schema, events, scopes or API changes. Default behaviour is unchanged.
