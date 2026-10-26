- **Instance apps: your own other apps can sign people in with Campbooks.** An instance
  administrator registers one from the server, never over the web:
  `campbooks-server admin apps add --name Applicamp --redirect-uri https://… --scopes openid
  profile email offline_access …` prints its client id and a secret shown only that once; `apps
  list`, `apps rotate-secret` and `apps remove` do the rest. The app signs in with OpenID Connect
  (code with PKCE and its secret), gets at most the scopes it was registered with, and people
  allow it once on the consent screen, where it shows as added by this server. It is listed in
  Settings → Apps and integrations, where anyone can revoke it; removing it signs everyone out
  of it at once.
