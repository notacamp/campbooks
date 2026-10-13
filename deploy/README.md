# Self-hosting Campbooks

1. `cp .env.example .env` and set at least `POSTGRES_PASSWORD` and `CAMPBOOKS_PUBLIC_URL`.
2. `docker compose up -d` starts the server, Postgres (with pgvector) and the identity
   provider. The server answers both the web app (open `http://localhost:8080/`) and the
   API (`/api/v1/...`) on port 8080; the identity provider listens on port 8081.
3. Check it: `curl http://localhost:8080/api/v1/system/health` should return `{"status":"ok","database":"ok"}`.
4. Optional local AI: `docker compose --profile ai up -d`, then pull the models once:
   `docker compose exec ollama ollama pull qwen3:8b` (TEI downloads `BAAI/bge-m3` itself).
5. Sign-in: the identity provider (Rauthy) starts with everything else, the
   API trusts its access tokens out of the box (issuer derived from
   `CAMPBOOKS_IDP_URL`), and the web app signs you in through it. See
   "Identity provider" and "Web sign-in" below; to use another IdP, edit the
   `CAMPBOOKS_AUTH__*` and `CAMPBOOKS_BFF__*` lines of the `server` service in
   `compose.yaml`.
6. Upgrades: `docker compose pull && docker compose up -d`; migrations run on startup
   (the server's and the identity provider's).
7. Background work (maintenance today; mail sync and the AI pass as they land)
   runs inside the same container: the image runs `campbooks-server all`, the
   API plus the job worker. Jobs live in Postgres, so nothing is lost on restart. To run the
   worker separately, add a second service with the same image and env and
   `command: ["worker"]` (and `healthcheck: { disable: true }`: the health
   check probes the API, which a worker does not serve), and give the first
   one `command: ["api"]`. Worker settings are the `CAMPBOOKS_JOBS__*` block
   of `.env.example`.

CPU inference works but is slow. For the `decide` and `complete` roles, use a GPU
host or a hosted provider serving open-weight models (set its URL in `.env`).

## Identity provider

Sign-in is handled by [Rauthy](https://github.com/sebadob/rauthy), an OpenID Connect /
OAuth 2.1 server, running beside Campbooks (see `docs/adr/0004-oauth-dedicated-idp-bff.md`).
It needs no setup: three compose services take care of it.

| Service | Runs | Does |
| --- | --- | --- |
| `idp-init` | before `idp`, every `up` | Generates the secrets once (first boot), rewrites the config from `.env` |
| `idp` | always | The IdP. Issuer `${CAMPBOOKS_IDP_URL}/auth/v1/`, data in the `idp-data` volume |
| `idp-provision` | after `idp` is healthy, every `up` | Registers Campbooks' scopes and clients once, and saves the web client's secret for the server |

A fourth one-shot, `server-init`, prepares the `server-secrets` volume for the
Campbooks server (see "Web sign-in").

### First sign-in to the admin UI

1. Read the generated admin password (it is never written to the logs):

   ```sh
   docker compose run --rm idp-init cat /app/data/campbooks/initial-admin-password
   ```

2. Open `http://localhost:8081/auth/v1/admin` (or `${CAMPBOOKS_IDP_URL}/auth/v1/admin`)
   and sign in as `admin@localhost` (or `CAMPBOOKS_IDP_ADMIN_EMAIL`).
3. Admin accounts must use a passkey. The admin UI sends you to your account page:
   open **MFA**, choose **Register New Key**, confirm your password and register a passkey
   (a platform authenticator or a security key). Then open the admin UI again.
4. Change the password under **Password** in the account page. The generated one stays in
   the volume, so treat it as spent once you have changed it.

Passkeys are bound to the IdP's host name. Over plain HTTP they work on `localhost` only;
anywhere else, serve the IdP over HTTPS (below) before registering passkeys, because
changing the host name later invalidates them.

### Serving it on a real domain

Put the IdP behind your TLS-terminating reverse proxy (its own host name, e.g.
`auth.example.com`, proxied to port 8081) and set in `.env`:

```sh
CAMPBOOKS_IDP_URL=https://auth.example.com
```

then `docker compose up -d`. With an `https://` URL the IdP trusts `X-Forwarded-*` headers
from private networks; narrow that with `CAMPBOOKS_IDP_TRUSTED_PROXIES` if your proxy has a
fixed address. Clients registered on first boot keep the redirect URIs built from
`CAMPBOOKS_PUBLIC_URL` at that time; if the app's URL changes, edit `campbooks-web` in the
admin UI (**Clients**).

Outgoing mail (password resets, invitations) is off until you add Rauthy's `SMTP_*`
variables to the `idp` service.

### What is in the `idp-data` volume

| Path | What |
| --- | --- |
| `secrets.toml` | Encryption key and cluster secrets. Without it the database cannot be read |
| `config.toml` | Generated from `.env` on every start. Do not edit |
| `campbooks/api-key` | Admin API key used by `idp-provision` (clients, scopes, client secrets) |
| `campbooks/initial-admin-password` | The first admin password |
| `campbooks/provisioned/<client>` | Markers: the client was set up, never touch it again |
| everything else | Rauthy's embedded database, with a nightly local backup in `state_machine/backups/` |

Back up the whole volume, for example:

```sh
docker run --rm -v campbooks_idp-data:/data -v "$PWD":/backup busybox:1.37.0-musl \
  tar czf /backup/idp-data.tgz -C /data .
```

### Rotating secrets

- **Admin password:** change it in the account page.
- **Web client secret:** admin UI → **Clients** → `campbooks-web` → generate a new secret, then
  `docker compose run --rm server-init rm /secrets/campbooks-web.client-secret` and
  `docker compose up -d`: `idp-provision` saves the new one, and the server reads it again
  the next time the IdP refuses the old one (no restart).
- **Admin API key:** admin UI → **API Keys** → `campbooks` → new secret, then write the new
  token (`campbooks$<secret>`) to `campbooks/api-key` in the volume.
- **Encryption key:** generate one with `docker compose run --rm --no-deps idp generate-enc-key`,
  add it to `secrets.toml` *next to* the old one and make it `key_active`
  (`docker compose run --rm idp-init vi /app/data/secrets.toml`), restart, migrate existing
  data to it from the admin UI (**Config → Encryption Keys**), and only then remove the old key.
- **Signing keys (JWKS)** rotate automatically every month; old keys stay published until
  tokens signed with them expire.

To re-apply Campbooks' default settings to a client, delete it in the admin UI, then
`docker compose run --rm idp-init rm /app/data/campbooks/provisioned/<client-id>` and
`docker compose up -d`.

## Web sign-in

The server signs browsers in itself (a backend-for-frontend, ADR 0004): open the app, and
if you are not signed in it sends you to the identity provider and back. The browser only
ever gets one `HttpOnly`, `SameSite=Strict` session cookie; the tokens stay on the server,
encrypted, in Postgres (schema `bff`). Any account of the identity provider can sign in,
including the first admin; create more under **Users** in its admin UI.

Sessions end after 48 hours unused or 30 days at most, when you sign out, or when your
identity-provider session ends (back-channel logout). The cookie is `Secure` and
`__Host-`-prefixed when `CAMPBOOKS_PUBLIC_URL` is `https://`; over plain `http://` (fine on
`localhost` only) it is neither, and the server logs a warning.

| `server-secrets` volume | What |
| --- | --- |
| `secret-key` | `CAMPBOOKS_SECRET_KEY`: encrypts the web sessions' tokens. Generated on first boot |
| `campbooks-web.client-secret` | Secret of the web app's OIDC client, written by `idp-provision` |

The server mounts this volume read-only (through group 10001); it never sees the
`idp-data` volume. Both files are regenerated if missing, so the volume needs no backup:
losing it only signs everyone out.

- **Instance secret key:** `docker compose run --rm server-init rm /secrets/secret-key`,
  then `docker compose up -d && docker compose restart server`. Everyone has to sign in to
  the web app again; nothing else is lost. (Without compose, set `CAMPBOOKS_SECRET_KEY` to
  `openssl rand -base64 32`.)
- **Back-channel logout** needs an `https://` `CAMPBOOKS_PUBLIC_URL` that the `idp`
  container can reach: Rauthy only calls https URLs. `idp-provision` registers
  `${CAMPBOOKS_PUBLIC_URL}/bff/backchannel-logout` for new installs; override it with
  `CAMPBOOKS_BACKCHANNEL_LOGOUT_URL`. On plain http there is none, and sessions end by
  sign-out or timeout.
- **Signing out** of the web app ends its session and revokes its refresh token, then sends
  you to the identity provider to end that session too (Rauthy asks you to confirm).

Settings (all optional; defaults shown in `.env.example`) are the `CAMPBOOKS_BFF__*` block.

### Developing against the IdP

The API runs on the host in development (`compose.dev.yaml` only starts Postgres). To get
the same IdP:

```sh
cp deploy/.env.example deploy/.env      # the defaults are fine for this
docker compose -f deploy/compose.yaml up -d idp-provision
```

That starts only `server-init`, `idp-init`, `idp` and `idp-provision`, at
`http://localhost:8081`. Set `CAMPBOOKS_PUBLIC_URL` in `deploy/.env` to where you open the
app (`http://localhost:8080` for `cargo run`, `http://localhost:5173` for the Vite dev
server) before the first `up`: it becomes the web client's redirect URI. Then run the server
with `CAMPBOOKS_AUTH__ISSUER=http://localhost:8081/auth/v1/`, the same
`CAMPBOOKS_PUBLIC_URL`, `CAMPBOOKS_SECRET_KEY` (any `openssl rand -base64 32`) and
`CAMPBOOKS_BFF__CLIENT_SECRET` (read it with
`docker compose -f deploy/compose.yaml run --rm --no-deps server-init cat /secrets/campbooks-web.client-secret`);
`clients/README.md` has the full recipe.
