# Proposal

## Why

The production cutover (campbooks-cloud#94) reuses OAuth apps that already
exist. The Google app has exactly one registered redirect URI,
`https://<prod host>/oauth/gmail/callback`, and changing a verified app's
redirect URIs is slow and risky. Campbooks Next serves its sign-in callback at
`/auth/callback/google` and always sends `<public url>/auth/callback/google`
as `redirect_uri`. Google requires the `redirect_uri` of the authorization
request to be registered and the token request to repeat it exactly, so the
two cannot meet.

## What Changes

- New optional setting `CAMPBOOKS_GOOGLE__REDIRECT_PATH`, default
  `/auth/callback/google`. It is the path Campbooks puts after its public URL
  in the `redirect_uri` it sends to Google, in the authorization request and
  in the code exchange. The route Campbooks serves does not change.
- The value must be an absolute path (`/` first, no scheme, host, query,
  fragment or dot segments). Anything else stops the server at startup with
  a message naming the setting.
- The same pattern for every sign-in provider, one function
  (`upstream::redirect_uri`): `CAMPBOOKS_<PROVIDER>__REDIRECT_PATH`, default
  `/auth/callback/<provider>`. Only Google exists today.
- `deploy/README.md` and `deploy/.env.example` document it: behind a reverse
  proxy that rewrites the pre-registered path to `/auth/callback/<provider>`.

## Non-goals

- No second route and no proxy configuration shipped in this repository.
- No change to the mailbox-connect callback (`/connections/google/callback`),
  which is a different redirect URI of the same OAuth app.
- No Microsoft or Zoho provider (the S2 slice adds them and reuses the helper).
