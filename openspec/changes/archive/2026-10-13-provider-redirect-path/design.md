# Design

## Behaviour

`upstream::redirect_uri(provider, issuer, configured)` returns
`<issuer><path>`, where the path is `configured` (trimmed, blank means unset)
or `/auth/callback/<provider>`. The Google client is built with that string and
uses the one value for the authorization request and for the token exchange,
so they cannot drift. Nothing else derives a redirect from the route. The
validation runs when the dependencies are read from the environment, so a bad
value is a startup error naming `CAMPBOOKS_GOOGLE__REDIRECT_PATH`.

Data ownership, events and scopes: none. The setting is read-only
configuration of the identity module; no table, event or scope changes. The
callback route is unchanged, and so are its OpenAPI entry and security.

## Alternatives considered

1. **A setting for the sent path, proxy rewrites the incoming path (chosen).**
   One optional setting and one function; the route table stays the single
   place that says what Campbooks serves. Cost: the operator needs a proxy
   rewrite rule. That is the situation in production anyway, because the old
   path reaches the proxy first, and a self-hoster with a fresh OAuth app
   leaves the setting unset and needs nothing.
2. **Register an extra route alias** (`/oauth/gmail/callback` served by
   Campbooks itself). No proxy rule, but the application would carry a
   deployment's legacy URL: a hard-coded alias is Rails-migration baggage for
   everyone, and a configurable alias means dynamic routes built from
   settings, which the OpenAPI catalogue and `cargo xtask openapi --check`
   do not model. Rejected.
3. **Derive the redirect from the request** (forwarded path). Clever, and
   wrong: the redirect URI must be the registered one, not what a request
   claims, and the token request has no request to copy it from.
4. **Re-register the redirect URI in the OAuth app.** The cleanest end state,
   and still possible later, but it is a console change on a verified app that
   the cutover should not depend on. Setting the path is removable once done.

Copying a pattern from the old Rails app on `main`: no. The Rails app registers
`/oauth/gmail/callback` as its own route; Next deliberately does not.

## Risks

A wrong path fails closed: Google refuses the authorization request
(`redirect_uri_mismatch`) and nobody signs in until it is fixed.
