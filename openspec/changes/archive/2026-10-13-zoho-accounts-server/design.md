## Design
`Endpoints::zoho(accounts_server)` builds the authorize URL from the given server and rejects anything outside `ZOHO_ACCOUNTS_SERVERS`. `deps.rs` reads `accounts_server` with the other provider settings and maps a failure to an error naming `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER`. The callback path is untouched.

Data ownership, events and scopes: none added.

## Alternatives considered
1. **Setting that must equal an allowlist entry (chosen).** One optional variable, validated at load, reusing the allowlist already trusted for callbacks. Cost: operators with a single-DC app must set it. Simple, self-host friendly, no new machinery, one source of truth for the allowed servers.
2. **Free-form authorize URL.** Most flexible, but an arbitrary host in config widens what the server can be pointed at and splits trust between two lists. Rejected.
3. **Derive from `CAMPBOOKS_IDP_ZOHO_ISSUER`** (the Rauthy-era setting that already names a data centre). Reuses an existing knob but couples the in-process issuer to the legacy IdP that is being removed. Rejected.
4. **Discover the data centre per user** (try each, or ask first). Works for single-DC apps without config but needs an extra screen or probing requests, and Zoho rejects the client at the wrong server anyway. Overkill for a deployment-time fact.

Does this copy a pattern from the old Rails app on `main`? No.
