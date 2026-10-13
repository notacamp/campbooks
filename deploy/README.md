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

8. Files (message bodies, attachments, documents) are stored outside Postgres,
   in the `blobs-data` volume by default. See "Blob storage" below to use an
   S3-compatible bucket instead.

CPU inference works but is slow for `complete`. Use a GPU host or a hosted provider
serving open-weight models for it (set its URL in `.env`). `decide` (Laya) and `embed`
run fine on CPU.

## Local AI

`docker compose --profile ai up -d` adds three services, one per AI role it serves
(see `docs/adr/0005-open-weight-ai-by-role.md` and `0009-decide-role-typed-decision-model.md`):

| Service | Role | Model | Notes |
| --- | --- | --- | --- |
| `laya` | `decide` | `convaiinnovations/laya-multilingual` (Apache-2.0, 322M) | Built from `deploy/laya`; ~680 MB download on first start; about 2 GB of RAM |
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
- **The image.** There is no official image, so compose builds one from
  `deploy/laya/Dockerfile`: Python slim, CPU-only PyTorch, `laya[serve]` and its
  libraries at pinned versions, running as a non-root user. On first start it
  downloads the multilingual checkpoint (at the commit the `laya` package lists as
  reviewed) into the `laya-models` volume; later starts load it from there. Build it
  again after an upgrade with `docker compose --profile ai build laya`. The image is
  optional and is not scanned by the release pipeline (only the Campbooks image is).
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
