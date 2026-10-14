# Self-hosting Campbooks

1. `cp .env.example .env` and set at least `POSTGRES_PASSWORD` and `CAMPBOOKS_PUBLIC_URL`.
2. `docker compose up -d` starts the server, Postgres (with pgvector) and the identity
   provider. The server answers both the web app (open `http://localhost:8080/`) and the
   API (`/api/v1/...`) on port 8080; the identity provider listens on port 8081.
3. Check it: `curl http://localhost:8080/api/v1/system/health` should return `{"status":"ok","database":"ok"}`.
4. Optional local AI: set `CAMPBOOKS_AI__DECIDE__API_KEY` in `.env`
   (`openssl rand -hex 32`), run `docker compose --profile ai up -d`, then pull the
   `complete` model once: `docker compose exec ollama ollama pull qwen3:8b`. Laya and
   TEI download their models themselves. See "Local AI" below.
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

8. Files (message bodies, attachments, documents) are stored outside Postgres,
   in the `blobs-data` volume by default. See "Blob storage" below to use an
   S3-compatible bucket instead.
9. Workspaces: the first person to sign in creates the first workspace and
   invites the others from Settings → Members. Who else may create a
   workspace is `CAMPBOOKS_SIGNUP__POLICY` (`open` by default; `invite_only`,
   `approval`, `code`; administrators decide `approval` requests and mint single-use `code`
   codes in Admin → Signups, see "Administrators"). Invitations are not emailed yet: the inviter gets the
   link once, right after inviting, to send on; it is built from
   `CAMPBOOKS_PUBLIC_URL`, so set that to the address people use. The new
   `workspaces:*` scopes are registered by `idp-provision` on the next `up`;
   sign-in clients provisioned before this release keep their old scope list
   until you add the scopes to them in the IdP admin UI (or re-provision them,
   see "Identity provider").
10. Workspace isolation: every workspace's rows are guarded by Postgres
   row-level security as well as by the queries. The server queries as a
   dedicated login that cannot bypass the policies, and refuses to start if
   it could. With the bundled Postgres this is automatic; see "Database
   logins" below for your own Postgres.

CPU inference works but is slow for `complete`. Use a GPU host or a hosted provider
serving open-weight models for it (set its URL in `.env`). `decide` (Laya) and `embed`
run fine on CPU.

## Database logins

Postgres skips row-level security for superusers and `BYPASSRLS` roles, and
a query that can `RESET ROLE` can climb back to the login's own power. So the
server uses two logins:

| Login | Setting | Used for | Must be |
|---|---|---|---|
| runtime | `CAMPBOOKS_DATABASE_URL` (+ optional `CAMPBOOKS_DATABASE_PASSWORD_FILE`) | every request and job | `LOGIN`, not superuser, no `BYPASSRLS`, owns nothing, member of `campbooks_app` |
| migration | `CAMPBOOKS_DATABASE_MIGRATION_URL` | applying migrations on startup, nothing else | the schema owner (or a superuser) |

**Bundled Postgres (compose).** Nothing to do. The one-shot `db-init` service
creates the `campbooks_runtime` login on every `up` (idempotent; it also
upgrades an existing installation), generates its password once into the
`db-secrets` volume and the server reads it from `/run/db/runtime-password`.
The owner login stays `campbooks` / `POSTGRES_PASSWORD`, used for migrations
only. To rotate the runtime password, delete the file in the volume
(`docker compose run --rm --entrypoint rm db-init /run/db/runtime-password`)
and `docker compose up -d`.

**Your own or a managed Postgres.** Create the login once, as an administrator
(the migrations then grant `campbooks_app` what each table needs):

```sql
CREATE ROLE campbooks_app NOLOGIN;            -- skip if it exists
CREATE ROLE campbooks_runtime LOGIN PASSWORD '...' NOSUPERUSER NOBYPASSRLS
  NOCREATEDB NOCREATEROLE;
GRANT campbooks_app TO campbooks_runtime;
-- Allow the migration user to run the first migration's grants:
GRANT campbooks_app TO <migration user>;      -- skip for a superuser
```

Then set `CAMPBOOKS_DATABASE_URL=postgres://campbooks_runtime:<password>@host/db`
(or omit the password and point `CAMPBOOKS_DATABASE_PASSWORD_FILE` at a file
holding it) and `CAMPBOOKS_DATABASE_MIGRATION_URL` to the owner's URL. On
managed services the "owner" is the user that created the database; do not
reuse it as the runtime login.

**Startup checks.** The server refuses to start (and says why) when the role
it queries as is a superuser, has `BYPASSRLS`, is a member of such a role, or
owns tables with row-level security; and, when a migration login is
configured, when the runtime login could still switch to a privileged role.
Only the old single-URL setup (a privileged login that `SET ROLE campbooks_app`)
keeps working, with a warning to split the logins. If you cannot yet, set
`CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS=true` to start anyway; workspace
isolation then rests on the queries alone.

## Local AI

`docker compose --profile ai up -d` adds three services, one per AI role it serves
(see `docs/adr/0005-open-weight-ai-by-role.md` and `0009-decide-role-typed-decision-model.md`):

| Service | Role | Model | Notes |
| --- | --- | --- | --- |
| `laya` | `decide` | `convaiinnovations/laya-multilingual` (Apache-2.0, 322M) | `ghcr.io/notacamp/campbooks-laya`; ~680 MB download on first start; about 2 GB of RAM |
| `ollama` | `complete` (and `vision`) | `qwen3:8b` | Pull models with `docker compose exec ollama ollama pull …` |
| `embeddings` | `embed` | `BAAI/bge-m3` | linux/amd64 only; on arm64 use Ollama's `bge-m3` |

### Laya (`decide`)

`decide` answers typed questions about an item — which option, what score, is this
true — with a probability for each answer. Laya is a small decision model that does
this in one forward pass, so it runs on CPU (a few hundred milliseconds per item on a
recent laptop).

- **API key, required.** `laya-serve` accepts unauthenticated requests when it has no
  key, so the `laya` service refuses to start until `CAMPBOOKS_AI__DECIDE__API_KEY` is
  set in `.env`. The server sends the same value. The service has no published port:
  only containers on the compose network reach it.
- **The image.** Laya has no official image, so the release pipeline publishes one
  next to the Campbooks image: `ghcr.io/notacamp/campbooks-laya`, built from
  `deploy/laya/Dockerfile` for linux/amd64 and linux/arm64, scanned and attested
  like the server image and published under the same tags (`next`, `next-<sha>`,
  `next-vX.Y.Z`), so pin both to the same tag. It is Python slim, CPU-only
  PyTorch, `laya[serve]` and its libraries at pinned versions, running as a
  non-root user. The checkpoint is not in the image: on first start it downloads
  the multilingual checkpoint (at the commit the `laya` package lists as reviewed)
  into the `laya-models` volume; later starts load it from there. To build the
  image yourself, swap `image:` for the commented `build: ./laya` line in
  `compose.yaml` and run `docker compose --profile ai build laya`.
- **CPU threads.** Inference uses every core by default; set `LAYA_THREADS` on the
  `laya` service to cap it.
- **Another model.** `CAMPBOOKS_AI__DECIDE__MODEL` also accepts
  `convaiinnovations/laya` (English only, 512 tokens) and
  `convaiinnovations/laya-typed-decisions`; the image preloads only the multilingual
  one, so set `LAYA_MODELS` on the service too.
- **An instruct model instead.** Set `CAMPBOOKS_AI__DECIDE__PROTOCOL=openai` and point
  `decide` at any OpenAI-compatible endpoint (Ollama, vLLM, a hosted provider). It
  answers the same questions with JSON-schema output. Probabilities come from token
  log-probabilities when the server returns them; without them answers carry no
  confidence, so nothing that needs one acts on its own.
- **Calibration.** Out of the box the models are over-confident. Per question type
  (and option count) temperatures in `CAMPBOOKS_AI__DECIDE__CALIBRATION__*` soften
  the probabilities (see `.env.example`); they default to 1.0 and will later be
  refitted from users' corrections.

## Blob storage

Raw messages, attachments and files live in object storage; Postgres keeps their
metadata and who uses them. Each file is stored once per workspace, under its SHA-256,
however many messages carry it. A file nothing uses any more is deleted by an hourly
sweep once `CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS` (default 24) have passed. Files larger
than `CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES` (default 50 MiB) are refused.

**Local disk (default).** `CAMPBOOKS_BLOBS__BACKEND=fs` stores files under
`CAMPBOOKS_BLOBS__PATH` (`/data/blobs`, the `blobs-data` volume). Writes are atomic: a
temporary file, flushed to disk, then renamed into place.

**S3-compatible store.** Any store that speaks the S3 API works (AWS S3, Garage, MinIO,
Ceph, Cloudflare R2, ...). Create a private bucket and an access key limited to it, then
set in `.env`:

```sh
CAMPBOOKS_BLOBS__BACKEND=s3
CAMPBOOKS_BLOBS__S3__BUCKET=campbooks-blobs
CAMPBOOKS_BLOBS__S3__ENDPOINT=https://s3.example.com   # unset for AWS
CAMPBOOKS_BLOBS__S3__REGION=us-east-1                  # what your store expects
CAMPBOOKS_BLOBS__S3__ACCESS_KEY_ID=...
CAMPBOOKS_BLOBS__S3__SECRET_ACCESS_KEY=...
CAMPBOOKS_BLOBS__S3__PATH_STYLE=true                   # most self-hosted stores
```

The server checks that it can reach the bucket on startup and refuses to start
otherwise. Large uploads go up in parts; add a lifecycle rule to the bucket that aborts
incomplete multipart uploads after a day, so a crash mid-upload leaves nothing behind.
The `blobs-data` volume is unused with this backend.

**Encryption at rest.** Campbooks does not encrypt files itself yet. Use your store's
encryption: server-side encryption on the bucket (SSE-S3 or SSE-KMS on AWS; Garage,
MinIO and Ceph have their own), or an encrypted disk under `blobs-data` for the local
backend.

**Moving between backends.** Copy the whole tree (`<workspace>/<aa>/<sha256>` keys,
the `_staging/` prefix excepted) to the new store with the server stopped, then switch
the settings: the keys are identical on disk and in a bucket.

## Backups

Back up these together, at about the same time:

| What | Where | Holds |
| --- | --- | --- |
| Postgres | `postgres-data` volume (use `pg_dump`) | Everything except files and the IdP's state |
| Files | `blobs-data` volume, or your bucket | Raw messages, attachments, documents |
| Identity provider | `idp-data` volume | Accounts, sign-in clients and the IdP's encryption key |

Restore Postgres and the files from the same point in time. A file that is missing from
the store while Postgres still lists it cannot be read, so take the database dump first,
then copy the files: files uploaded in between are only harmless extras. For a bucket,
turn on versioning (and keep old versions for a few days) instead of copying.

For the local volume, for example:

```sh
docker run --rm -v campbooks_blobs-data:/data -v "$PWD":/backup busybox:1.37.0-musl \
  tar czf /backup/blobs-data.tgz -C /data .
```

## Monitoring

Out of the box Campbooks exports nothing and opens no extra port. It writes JSON logs to
stdout (`docker compose logs server`); every line inside a request or a job carries its
`trace_id`, and each request ends with one `request finished` line giving the route, the
status and the latency. `/api/v1/system/health` and `/api/v1/system/version` work as
before.

**A Prometheus page.** Set an address for a separate metrics listener:

```sh
CAMPBOOKS_TELEMETRY__METRICS_ADDR=0.0.0.0:9464
```

and scrape `http://server:9464/metrics` from a Prometheus (or Grafana Alloy, VictoriaMetrics,
Netdata, ...) on the same Docker network. The listener has no authentication: do not
publish the port (the compose file does not), and never expose it to the internet.

**OTLP to a collector.** Point the server at any OTLP/HTTP receiver (an OpenTelemetry
Collector, Grafana Alloy, Jaeger, Tempo, a hosted service):

```sh
CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT=http://collector:4318   # /v1/traces and /v1/metrics are appended
CAMPBOOKS_TELEMETRY__OTLP_HEADERS=authorization=Bearer abc # optional, secret
CAMPBOOKS_TELEMETRY__SAMPLE_RATIO=1                        # share of new traces kept, 0 to 1
```

Metrics are pushed every 60 seconds, traces in batches. A collector that is down costs
dropped telemetry, never slower requests. The standard `OTEL_SERVICE_NAME` and
`OTEL_RESOURCE_ATTRIBUTES` variables are honoured (e.g.
`OTEL_RESOURCE_ATTRIBUTES=deployment.environment=production`). Both exports can be on at
once.

**What you get.**

| Metric (Prometheus name) | What | Labels |
| --- | --- | --- |
| `http_server_request_duration_seconds` | Requests: rate, errors (by status) and latency | `http_request_method`, `http_route` (the route template), `http_response_status_code`, `error_type` (the error's `code`, e.g. `not_found`) |
| `campbooks_job_duration_seconds` | Job attempts and how they ended | `job_kind`, `job_queue`, `job_outcome` (`succeeded`, `retried`, `failed`, `lost`) |
| `campbooks_job_wait_seconds` | How long a ready job waited for a worker | `job_kind`, `job_queue` |
| `campbooks_jobs` | Jobs per queue and state, sampled every 15 s | `job_queue`, `job_state` (`ready`, `scheduled`, `running`, `failed`) |
| `campbooks_jobs_oldest_ready_age_seconds` | Age of the oldest job waiting for a worker | `job_queue` |
| `gen_ai_client_operation_duration_seconds` | AI provider calls and their errors | `gen_ai_operation_name`, `campbooks_ai_role`, `server_address`, `gen_ai_request_model`, `error_type` |
| `gen_ai_client_token_usage` | Tokens per AI call | the above plus `gen_ai_token_type` |
| `db_client_connection_count`, `db_client_connection_max` | Database pool use | `db_client_connection_state` |
| `db_client_connection_wait_time_seconds` | Time waiting for a pooled connection | |
| `campbooks_build_info` | Always 1 | `service_version`, `campbooks_role` |

Good first alerts: a rising share of 5xx in `http_server_request_duration_seconds_count`,
`campbooks_jobs{job_state="failed"}` going up, `campbooks_jobs_oldest_ready_age_seconds`
above a few minutes (workers cannot keep up: add one, see step 7 above), and
`db_client_connection_count{db_client_connection_state="used"}` sitting at
`db_client_connection_max`.

Traces have a span per request (named after its route), per job attempt (linked to the
request that enqueued it), per AI call and per identity-provider fetch, and honour a W3C
`traceparent` sent by a client or proxy. If your proxy faces the internet, strip that
header there unless you want callers to choose which of their requests are traced.

**Privacy.** Telemetry never contains message content, prompts, tokens, cookies, codes
or email addresses, and metrics never carry ids (workspace ids appear on spans only). A
test in the server enforces this for every release.

## Identity provider

Sign-in is handled by [Rauthy](https://github.com/sebadob/rauthy), an OpenID Connect /
OAuth 2.1 server, running beside Campbooks (see `docs/adr/0004-oauth-dedicated-idp-bff.md`).
It needs no setup: three compose services take care of it.

| Service | Runs | Does |
| --- | --- | --- |
| `idp-init` | before `idp`, every `up` | Generates the secrets once (first boot), rewrites the config from `.env` |
| `idp` | always | The IdP. Issuer `${CAMPBOOKS_IDP_URL}/auth/v1/`, data in the `idp-data` volume |
| `idp-provision` | after `idp` is healthy, every `up` | Registers Campbooks' scopes and clients once, saves the web client's secret for the server, and registers the sign-in providers you configure (see "Sign in with Google, Microsoft or Zoho") |

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
| `campbooks/api-key` | Admin API key used by `idp-provision` (clients, scopes, client secrets, sign-in providers) |
| `campbooks/initial-admin-password` | The first admin password |
| `campbooks/provisioned/<client>` | Markers: the client was set up, never touch it again |
| `campbooks/provisioned/provider-<name>` | Markers: the sign-in provider's id and a fingerprint of its settings |
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
| `secret-key` | `CAMPBOOKS_SECRET_KEY`: encrypts the web sessions' tokens and connected accounts' credentials. Generated on first boot |
| `campbooks-web.client-secret` | Secret of the web app's OIDC client, written by `idp-provision` |

The server mounts this volume read-only (through group 10001); it never sees the
`idp-data` volume. Both files are regenerated if missing, so the volume needs no backup:
losing it signs everyone out and asks members to connect their accounts again.

- **Instance secret key:** `docker compose run --rm server-init rm /secrets/secret-key`,
  then `docker compose up -d && docker compose restart server`. Everyone has to sign in to
  the web app again, and connected accounts show "needs reauthorisation" until their member
  connects them again; nothing else is lost. (Without compose, set `CAMPBOOKS_SECRET_KEY` to
  `openssl rand -base64 32`.)
- **Back-channel logout** needs an `https://` `CAMPBOOKS_PUBLIC_URL` that the `idp`
  container can reach: Rauthy only calls https URLs. `idp-provision` registers
  `${CAMPBOOKS_PUBLIC_URL}/bff/backchannel-logout` for new installs; override it with
  `CAMPBOOKS_BACKCHANNEL_LOGOUT_URL`. On plain http there is none, and sessions end by
  sign-out or timeout.
- **Signing out** of the web app ends its session and revokes its refresh token, then sends
  you to the identity provider to end that session too (Rauthy asks you to confirm).

Settings (all optional; defaults shown in `.env.example`) are the `CAMPBOOKS_BFF__*` block.

## Built-in sign-in (Campbooks as its own identity provider)

Campbooks is becoming its own identity provider (ADR 0014): it signs people in with
Google, Microsoft, Zoho or a passkey and issues every token itself, so the `idp`
services above will go away. The built-in issuer is already in the server. It runs when
`CAMPBOOKS_AUTH__ISSUER` is **unset** (or names `CAMPBOOKS_PUBLIC_URL` itself) and
`CAMPBOOKS_SECRET_KEY` is set. The shipped `compose.yaml` still points
`CAMPBOOKS_AUTH__ISSUER` at Rauthy, so nothing changes for an existing install until the
switch-over release removes Rauthy.

With the built-in issuer:

- The issuer is `CAMPBOOKS_PUBLIC_URL` (no path). Its metadata is at
  `/.well-known/openid-configuration` (and `/.well-known/oauth-authorization-server`), its
  keys at `/oauth2/jwks`, and its endpoints under `/oauth2/*`. The web app signs in through
  it exactly as through Rauthy, with no client secret to provision.
- It signs with Ed25519 keys it generates on first start and rotates every 30 days,
  stored in Postgres (schema `identity`) encrypted with `CAMPBOOKS_SECRET_KEY`. A server
  whose secret key does not open them refuses to start ("CAMPBOOKS_SECRET_KEY does not
  match the stored signing keys"): restore the right key. To replace the secret key on
  purpose, also delete the rows of `identity.signing_keys`; everyone signs in again.
- The public URL must be `https://` (plain `http://` only with `CAMPBOOKS_ENV=development`),
  so the sign-in cookies are always `Secure`.
- New accounts follow `CAMPBOOKS_SIGNUP__POLICY`: on `invite_only`, only people whose
  Google address has a pending invitation get an account.
- **Sign in with Google** uses the same Google OAuth client as connected accounts (below).
  Add the redirect URI `${CAMPBOOKS_PUBLIC_URL}/auth/callback/google` to it, and the
  `profile` scope to its consent screen. A new person's sign-in asks, in the same consent,
  for their mail: once they create their first workspace, that mailbox is connected
  without asking again. Until Google verifies your app for the Gmail scope, people see
  Google's "unverified app" screen when they sign up. Publish the app (rather than leaving it
  in "Testing"): testing apps issue refresh tokens that expire after 7 days.
- **An OAuth app that registered another callback path.** Google only accepts a
  `redirect_uri` registered in the app, and the token request must repeat it. If the app
  already has a different path (for example an earlier deployment's
  `https://campbooks.example.com/oauth/gmail/callback`), set
  `CAMPBOOKS_GOOGLE__REDIRECT_PATH=/oauth/gmail/callback` and make your reverse proxy
  rewrite that path to `/auth/callback/google` (same query string). Campbooks then sends
  the registered URI in both requests and keeps serving `/auth/callback/google`. Leave it
  unset for a new OAuth app. Other providers follow the same pattern
  (`CAMPBOOKS_<PROVIDER>__REDIRECT_PATH`, default `/auth/callback/<provider>`). The value
  must be an absolute path with no host, query or fragment, or the server refuses to
  start.
- Accounts are linked to a provider account by its id, never by email address. Signing in
  with a provider account Campbooks doesn't know offers to create a new account.
- **Sign in with Microsoft** (personal, work and school accounts). In the Microsoft Entra
  admin center, register an app (**App registrations**, "Accounts in any organizational
  directory and personal Microsoft accounts"), add the **Web** redirect URI
  `${CAMPBOOKS_PUBLIC_URL}/auth/callback/microsoft`, create a client secret, and set
  `CAMPBOOKS_MICROSOFT__CLIENT_ID` and `CAMPBOOKS_MICROSOFT__CLIENT_SECRET_FILE`.
  `CAMPBOOKS_MICROSOFT__TENANT` (default `common`) narrows who may sign in:
  `organizations` (work and school only, for an app registered that way), `consumers`
  (personal only) or one tenant id. Microsoft does not verify the address it reports, so
  it counts as verified (for an invite-only instance) only when you also add the optional
  claim `xms_edov` to the ID token (**Token configuration**). Microsoft has no mailbox
  adapter yet: signing in with it connects no mail.
- **Sign in with Zoho**, in every Zoho data centre. In the Zoho API console, add a
  **Server-based application** with the redirect URI
  `${CAMPBOOKS_PUBLIC_URL}/auth/callback/zoho`, and turn on **Multi-DC** (Settings) so the
  same client works for people in every data centre. Without Multi-DC, set
  `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER` to your app's data centre (for an app created in the
  EU console, `https://accounts.zoho.eu`): the default, `https://accounts.zoho.com`, rejects
  such an app as an invalid client. It must be one of the servers below, as
  `https://<host>` with no path, or Campbooks refuses to start. Set `CAMPBOOKS_ZOHO__CLIENT_ID` and
  `CAMPBOOKS_ZOHO__CLIENT_SECRET_FILE`. Campbooks only talks to Zoho's own accounts
  servers (`accounts.zoho.com`, `.eu`, `.in`, `.com.au`, `.jp`, `.ca`, `.sa`, `.uk`), so a
  forged callback cannot send the code anywhere else. The same app connects Zoho Mail:
  see Connected accounts.
- **Behind a reverse proxy, set `CAMPBOOKS_TRUSTED_PROXIES`.** Sign-in, its callbacks,
  starting a link, the token endpoint, client registration and the consent screen are
  rate-limited per client address. Behind a proxy every request
  comes from the proxy, so without this setting all clients share one budget (30
  sign-ins a minute for everyone). Set it to the proxy's address or network, as
  comma-separated addresses or CIDR blocks, e.g. `CAMPBOOKS_TRUSTED_PROXIES=172.18.0.0/16`
  for the Docker network your proxy reaches the server over (`docker network inspect
  <network>` shows its subnet). Campbooks then reads `X-Forwarded-For` from those
  addresses only, taking the right-most entry that is not one of them; anything a client
  writes further left is ignored.
- **More ways in.** Signed in, a person adds another Google, Microsoft or Zoho account
  under Settings → Ways to sign in, and every mailbox they connect becomes a way in too.
  A provider account that is already another Campbooks account's way in is refused, never
  merged. The last way in can't be removed, and adding or removing one needs a sign-in in
  the last ten minutes. Disconnecting a mailbox keeps its sign-in.
- **Apps other than the web app.** Every one of them gets in only after the person
  allows it on Campbooks' consent screen, which names the app, says whether anyone
  vouches for it, lists what it will be able to do and asks which workspace it may use.
  People see and revoke what they allowed under Settings → Apps and integrations.
  - **The desktop and mobile apps** (`campbooks-native`) sign in in the system browser
    with a loopback or `com.notacamp.campbooks:` redirect. They are asked for every
    time: any program on a device can listen on a loopback port.
  - **A workspace's own apps.** Owners and admins register them under Settings → Apps
    and integrations: a server-to-server app gets a client id and secret for the
    `client_credentials` grant (`POST /oauth2/token`), its tokens work in that workspace
    only, and the secret is shown once (rotate it if it is lost); a sign-in app gets a
    client id for its users to allow.
  - **MCP agents and other apps** find everything from the API's URL: a call without a
    token answers `WWW-Authenticate: Bearer resource_metadata="…/.well-known/oauth-protected-resource/api"`,
    which names this issuer. **Self-registration is off by default.** To let agents
    connect without anyone setting them up first, turn on
    `CAMPBOOKS_OAUTH__DYNAMIC_REGISTRATION=on` (they register at `/oauth2/register`:
    public clients only, unused ones deleted after 30 days) and/or
    `CAMPBOOKS_OAUTH__CLIENT_METADATA_DOCUMENTS=on` (an `https` URL of their metadata
    document is their client id). With both off, `/oauth2/register` answers 404, the
    issuer metadata lists no `registration_endpoint`, and such clients are unknown;
    turning one off later also stops the clients that used it. Agents can still use a
    sign-in app a workspace admin registers under Settings → Apps and integrations. The consent screen marks them unverified. Their
    tokens are bound to one workspace (they see no other, and can't create or join one)
    and can never manage accounts, apps or members. Registrations and document fetches
    are also capped per hour for the whole instance.
  - Access tokens now name `${CAMPBOOKS_PUBLIC_URL}/api` as their audience;
    `CAMPBOOKS_AUTH__AUDIENCE` only applies to an outside identity provider.
- **Passkeys.** Nothing to configure: people add passkeys under Settings → Ways to sign
  in, sign in with them on the sign-in screen (the passkey button, `P`, or the browser's
  autofill), and can create an account with a passkey alone. Passkeys belong to the host
  of `CAMPBOOKS_PUBLIC_URL` (the WebAuthn relying party): **changing that host makes every
  passkey stop working**, so people need another way in first. Browsers offer passkeys
  only over `https://`, or `http://localhost` in development; on any other plain-http
  host, or an IP address, the passkey options are hidden. Passkey-only accounts have no
  email address, so on an `invite_only` instance passkeys can't create accounts (they
  still sign in, and can be added once you are in). A passkey that reports an
  authenticator counter going backwards (a possible clone) is allowed and recorded in
  `identity.audit_events` as `passkey_sign_count_regressed`. Passkey ceremonies are
  rate-limited per client address (set `CAMPBOOKS_TRUSTED_PROXIES` behind a proxy, as
  above), and at most 30 passkey-only accounts are created per hour on the instance.

## Administrators

An administrator is a person whose account has an **admin profile** (ADR 0015). They see
the **Admin** section of the app (account menu → Admin, or `/admin`): the System
panel, with the version, uptime, database and migrations, the effective configuration
(never a secret) and whether each AI role's provider answers; **People**, everyone who has
signed in, with their workspaces, how they sign in and a **Suspend** action;
**Workspaces**, with their members and each connected mailbox's health; and the
**Signups** panel (below). Jobs follow.

Profiles exist only through the server's command line, never through the app or the API,
so becoming an administrator needs shell access to the server. The command connects with
the migration login (`CAMPBOOKS_DATABASE_MIGRATION_URL`); the login every request uses
cannot write a profile.

**The first super admin.** Sign in to the app once, then on the server:

```sh
docker compose exec server campbooks-server admin grant --email you@example.com --permission super
```

It names the account it found and asks before changing anything (`--yes` skips the
question). `--email` must be a verified address of exactly one account; when two accounts
share it, or with another identity provider (the shipped `compose.yaml` still uses
Rauthy), pass `--account <id>` instead: the id is the `sub` that
`https://<your instance>/bff/session` shows to the signed-in person.

```sh
docker compose exec server campbooks-server admin list            # who, with what, since when
docker compose exec server campbooks-server admin revoke --email you@example.com   # all of it
docker compose exec server campbooks-server admin revoke --account <id> --permission jobs.retry
docker compose exec server campbooks-server admin log             # the audit trail
```

Permissions are `super` (everything), `system.read`, `signups.review`, `users.read`,
`users.suspend`, `workspaces.read`, `jobs.read` and `jobs.retry`; the dashboard shows only to
`super` for now. Changes apply on the person's next request.

**Signups.** With `CAMPBOOKS_SIGNUP__POLICY=approval`, someone new who tries to create a
workspace is refused and a request appears in Admin → Signups (permission `signups.review`);
approve or reject it there. They see the answer the next time they open the app; Campbooks
does not email them yet. With `CAMPBOOKS_SIGNUP__POLICY=code`, mint codes in the same panel:
each code is shown once when minted (copy it then), works for the number of uses you set (one
by default) until the day you pick, and can be revoked. Only a digest of a code is stored.
Codes in `CAMPBOOKS_SIGNUP__CODES` still work and stay reusable by anyone who has them; prefer
minted codes, and the setting needs no value when you only use minted ones.

- Every admin request and every command above is recorded (`admin log`).
- The admin API asks for a recent sign-in: more than 12 hours after signing in, the app asks
  the administrator to sign in again.
- Admin views show metadata only (counts, states, providers, addresses people signed in
  with), never messages or documents, and nobody can act as another person.
- Suspending someone (`users.suspend`) signs them out everywhere at once and stops every
  sign-in until they are unsuspended; nothing of theirs is deleted. An administrator cannot
  be suspended from the app: revoke their profile with `admin revoke` first.
- With another identity provider (`CAMPBOOKS_AUTH__ISSUER`), give the `admin:access` scope
  only to the web app's client, and make sure access tokens carry `auth_time`; without it
  every admin request asks to sign in again. Campbooks' built-in sign-in does both.

## Connected accounts

Members connect outside accounts (Gmail and Zoho Mail); Campbooks asks Google for consent through
its built-in sign-in (the same redirect URI, `/auth/callback/google`), keeps the grant
encrypted with the instance secret key (schema `connectors`), and
syncs mail on its own: every minute while an account is busy, backing off to every few
hours when nothing changes. Nothing is connected until you register an OAuth client:

1. In the Google Cloud console, create an OAuth client of type **Web application** and
   enable the **Gmail API** for its project.
2. Add the redirect URI `${CAMPBOOKS_PUBLIC_URL}/auth/callback/google`, e.g.
   `https://campbooks.example.com/auth/callback/google`. The earlier
   `/connections/google/callback` is no longer used; you can remove it.
3. Add the scopes `openid`, `email`, `profile` and
   `https://www.googleapis.com/auth/gmail.modify` to the consent screen. Gmail's scope is restricted: until Google verifies your app, only
   the test users you list can connect.
4. Set `CAMPBOOKS_GOOGLE__CLIENT_ID` and put the client secret in a file the server reads
   (`CAMPBOOKS_GOOGLE__CLIENT_SECRET_FILE`; `CAMPBOOKS_GOOGLE__CLIENT_SECRET` inline for
   development), then restart the server.

A connection belongs to the workspace and records the member who granted it. When that
member leaves the workspace, or the workspace is deleted, the grant is revoked at Google.
Tuning (all optional) is the `CAMPBOOKS_CONNECTORS__*` block of `.env.example`.

**Zoho Mail** uses the Zoho app of Sign in with Zoho (above): the same client, secret and
redirect URI (`/auth/callback/zoho`, or `CAMPBOOKS_ZOHO__REDIRECT_PATH` behind a proxy
that rewrites it). Signing in with Zoho then also asks for the mailbox, and members can
connect more Zoho logins. The app must allow the scopes `openid`, `email`, `profile`,
`ZohoMail.accounts.READ`, `ZohoMail.folders.READ` and `ZohoMail.messages.ALL` (a
server-based Zoho app takes its scopes with each request, so there is nothing more to
register). Each mail account of a Zoho login becomes its own
inbox. Campbooks reads and writes each account's mail at its own data centre
(`accounts.zoho.eu` → `mail.zoho.eu`), polls it (Zoho has no push), and keeps to
`CAMPBOOKS_CONNECTORS__ZOHO__REQUESTS_PER_MINUTE` per Zoho login. Zoho labels are not
synced yet.

Connecting needs the built-in sign-in: an install still on Rauthy (`CAMPBOOKS_AUTH__ISSUER`
set) cannot connect mail until it switches. `connectors.start_connection` answers with the
built-in sign-in's URL, which comes back to the app with `connected=<connection id>`.

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

## Sign in with Google, Microsoft or Zoho

Besides passkeys and passwords, the identity provider's login page can offer **Google**,
**Microsoft** and **Zoho**. Each one is optional and appears once you give it a client ID
and a client secret; `idp-provision` registers it in Rauthy on the next
`docker compose up -d`. Leave them unset and the login page offers passkey and password
only.

### 1. The redirect URI

All three send people back to the same address, which you register in each provider's
console:

```
${CAMPBOOKS_IDP_URL}/auth/v1/providers/callback
```

For example `https://auth.example.com/auth/v1/providers/callback`, or
`http://localhost:8081/auth/v1/providers/callback` on your own machine. Serve the IdP over
https before you invite anyone (see "Serving it on a real domain"); the providers only
accept plain http for `localhost`. `idp-provision` prints the exact URI in its log
(`docker compose logs idp-provision`).

### 2. Create the app in the provider's console

The scopes are `openid email profile` for all three; Campbooks asks for nothing else.

**Google** (Google Cloud console, `console.cloud.google.com`):

1. Pick or create a project. Open **APIs & Services → OAuth consent screen** (Google Auth
   Platform): app name, support email, audience **External**. Under **Data access** add
   `openid`, `.../auth/userinfo.email` and `.../auth/userinfo.profile` (non-sensitive, no
   review needed). Publish the app: while it is in *Testing*, only the listed test users can
   sign in.
2. **Clients** (or **Credentials**) → **Create client** → application type **Web
   application** → **Authorized redirect URIs**: the URI above. No JavaScript origins.
3. Put the **Client ID** in `.env` as `CAMPBOOKS_IDP_GOOGLE_CLIENT_ID` and the **Client
   secret** in `secrets/google-client-secret`.

**Microsoft** (Microsoft Entra admin center, `entra.microsoft.com`):

1. **App registrations → New registration**. Supported account types must match
   `CAMPBOOKS_IDP_MICROSOFT_TENANT`:

   | `CAMPBOOKS_IDP_MICROSOFT_TENANT` | Supported account types |
   | --- | --- |
   | `common` (default) | Accounts in any organizational directory and personal Microsoft accounts |
   | `organizations` | Accounts in any organizational directory (work and school only) |
   | `consumers` | Personal Microsoft accounts only |
   | your tenant ID | Accounts in this organizational directory only |

2. Redirect URI: platform **Web**, the URI above.
3. **Certificates & secrets → New client secret**. Copy its **Value** (not the Secret ID)
   into `secrets/microsoft-client-secret`. Secrets expire (at most two years): put a
   reminder in your calendar and rotate it (below) before then.
4. **API permissions**: the default Microsoft Graph `User.Read` is enough.
5. Put the **Application (client) ID** from **Overview** in `.env` as
   `CAMPBOOKS_IDP_MICROSOFT_CLIENT_ID`.

Register a new app rather than reusing an old one: multi-tenant apps created since June 2023
leave the email out of the sign-in when its domain is not verified by its owner, so nobody
can sign in as an address they do not own. For an older app, set `removeUnverifiedEmailClaim`
to `true` in its `authenticationBehaviors` (Microsoft Graph).

**Zoho** (the API console of your data centre: `api-console.zoho.eu` for the EU,
`api-console.zoho.com` for the US, and so on):

1. **Add client → Server-based Applications**.
2. Client name, homepage URL (your `CAMPBOOKS_PUBLIC_URL`), **Authorized redirect URIs**:
   the URI above. **Create**.
3. Put the **Client ID** in `.env` as `CAMPBOOKS_IDP_ZOHO_CLIENT_ID` and the **Client
   Secret** in `secrets/zoho-client-secret`. Set `CAMPBOOKS_IDP_ZOHO_ISSUER` to the
   accounts server of that data centre (default `https://accounts.zoho.eu`; also
   `https://accounts.zoho.com`, `https://accounts.zoho.in`, `https://accounts.zoho.com.au`,
   `https://accounts.zoho.jp`, ...).

People whose Zoho account lives in another data centre cannot sign in with Zoho here: Zoho
expects the sign-in to be finished at their own data centre, which the IdP does not do.

### 3. Put the secret in place and start

Secrets are files in `deploy/secrets/` (ignored by git), mounted read-only into
`idp-provision`, which runs as uid 10001; never put them in `.env`.

```sh
cd deploy
nano secrets/google-client-secret     # paste the secret on one line, save
sudo chown 10001:10001 secrets/google-client-secret
sudo chmod 0400 secrets/google-client-secret
```

(Docker Desktop on macOS or Windows does not need the `chown`.) Set the client ID in
`.env`, then `docker compose up -d` and check `docker compose logs idp-provision`. Open the
app: the login page now shows a button per provider (with its name; upload a logo under
**Providers** in the admin UI if you want one).

**Installs from before sign-in providers existed:** the IdP's `campbooks` API key may not
manage providers yet, and `idp-provision` stops with a message saying so (everything else is
already provisioned). Grant it once: admin UI → **API Keys** → `campbooks` → **Auth
Providers**: read, create, update. Then `docker compose up -d` again.

### Who can get in

Whether someone without an account may create one by signing in with a provider follows
`CAMPBOOKS_SIGNUP__POLICY`, so a provider never opens an instance you closed:

| `CAMPBOOKS_SIGNUP__POLICY` | Someone new signs in with a provider |
| --- | --- |
| `open` | Gets an account and may create a workspace |
| `code` | Gets an account; creating a workspace needs a signup code |
| `approval` | Gets an account; asking for a workspace records a request, nobody is let in until it is approved |
| `invite_only` | Refused. An admin creates their account first (**Users** in the admin UI); once they have set a password or passkey and signed in, they link the provider from their account page |

`CAMPBOOKS_IDP_PROVIDER_SIGNUP=open` lets new people create accounts through a provider even
on an `invite_only` instance (they still need an invitation to join a workspace);
`closed` never lets them. The instance's first user may always create a workspace, so on a
fresh instance sign in yourself before you share the address.

### Accounts and linking

- An account is linked to **one** provider at most. The first sign-in with a provider
  creates the account (as above) and links it; later sign-ins with that provider land on
  the same account, even if the email there changes.
- If an account with the same email already exists (made with a password or passkey, or by
  another provider), signing in with a provider is refused ("already exists but is not
  linked"). Sign in the usual way, open your account page
  (`${CAMPBOOKS_IDP_URL}/auth/v1/account`) and link the provider there; the emails must
  match. Automatic linking by email is off on purpose: the IdP would link without checking
  that the provider verified the address.
- An account linked to Google cannot also sign in with Microsoft or Zoho. To switch, unlink
  it on the account page (it needs a password or passkey first) and link the other one.
- Microsoft does not say whether an email is verified, so accounts created through Microsoft
  start with an unverified email, and accepting a Campbooks invitation needs a verified one.
  An admin can mark it verified (**Users** → the user → E-Mail verified).

### Changing or removing a provider

- **Rotating a secret:** replace the file, `docker compose up -d`. A provider is configured
  again whenever its client ID, secret, tenant or issuer, or whether it creates accounts,
  changes; the log says so. Other edits made under **Providers** in the admin UI are kept
  until then, and replaced by Campbooks' settings when that happens.
- **Removing one:** remove its client ID from `.env`. It stays registered (deleting it would
  strand the accounts linked to it) and the log reminds you; disable or delete it under
  **Providers** in the admin UI. To have `idp-provision` set it up from scratch again, delete
  it there and remove its marker:
  `docker compose run --rm idp-init rm /app/data/campbooks/provisioned/provider-<google|microsoft|zoho>`.
