# Self-hosting Campbooks

1. `cp .env.example .env` and set at least `POSTGRES_PASSWORD` and `CAMPBOOKS_PUBLIC_URL`.
2. `docker compose up -d` starts the server, Postgres (with pgvector) and the identity
   provider. The server answers both the web app (open `http://localhost:8080/`) and the
   API (`/api/v1/...`) on port 8080; the identity provider listens on port 8081.
3. Check it: `curl http://localhost:8080/api/v1/system/health` should return `{"status":"ok","database":"ok"}`.
4. Optional local AI: `docker compose --profile ai up -d`, then pull the models once:
   `docker compose exec ollama ollama pull qwen3:8b` (TEI downloads `BAAI/bge-m3` itself).
5. Sign-in: the identity provider (Rauthy) starts with everything else, and the
   API trusts its access tokens out of the box (issuer derived from
   `CAMPBOOKS_IDP_URL`). See "Identity provider" below; to use another IdP,
   edit the two `CAMPBOOKS_AUTH__*` lines of the `server` service in `compose.yaml`.
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
| `idp-provision` | after `idp` is healthy, every `up` | Registers Campbooks' scopes and clients once |

The API does not check tokens yet; that lands with the resource-server work.

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
| `campbooks/campbooks-web.client-secret` | Secret of the web app's OIDC client |
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
  `docker compose run --rm idp-init rm /app/data/campbooks/campbooks-web.client-secret` and
  `docker compose up -d`: `idp-provision` saves the new one.
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

### Developing against the IdP

The API runs on the host in development (`compose.dev.yaml` only starts Postgres). To get
the same IdP:

```sh
cp deploy/.env.example deploy/.env      # the defaults are fine for this
docker compose -f deploy/compose.yaml up -d idp-provision
```

That starts only `idp-init`, `idp` and `idp-provision`, at `http://localhost:8081`.
