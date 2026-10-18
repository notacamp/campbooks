# Self-hosting Campbooks

1. `cp .env.example .env` and set at least `POSTGRES_PASSWORD` and `CAMPBOOKS_PUBLIC_URL`.
2. `docker compose up -d` starts the server and Postgres (with pgvector). The server
   answers both the web app (open `http://localhost:8080/`) and the API (`/api/v1/...`) on
   port 8080.
3. Check it: `curl http://localhost:8080/api/v1/system/health` should return `{"status":"ok","database":"ok"}`.
   `/up` is an alias that gives the same answer, for load balancers and uptime monitors that probe it.
4. Optional local AI: set `CAMPBOOKS_AI__DECIDE__API_KEY` in `.env`
   (`openssl rand -hex 32`), run `docker compose --profile ai up -d`, then pull the
   `complete` model once: `docker compose exec ollama ollama pull qwen3:8b`. Laya and
   TEI download their models themselves. See "Local AI" below.
5. Sign-in: Campbooks is its own identity provider (ADR 0014); nothing runs beside it.
   Register a Google, Microsoft or Zoho OAuth app (or use passkeys only) as described in
   "Built-in sign-in" below. Upgrading from a release that shipped Rauthy: see "Upgrading
   from the Rauthy sign-in".
6. Upgrades: `docker compose pull && docker compose up -d`; migrations run on startup
   (the server's). Campbooks needs PostgreSQL 15 or later and is built and tested on 17,
   the bundled image; the server refuses to start on an older one. A migration waits a
   few seconds for each lock it needs (a minute at most where it sets no limit of its
   own). Behind a long-running transaction, such as a web session's token refresh holding
   its row for up to ten seconds while a migration changes the sign-in tables, the
   migration fails, the server says so and exits, and it migrates again at its next start
   (compose restarts it). Nothing is half-applied: each migration is one transaction, and
   an index build it leaves unfinished is dropped and built again.
7. Background work (maintenance today; mail sync and the AI pass as they land)
   runs inside the same container: the image runs `campbooks-server all`, the
   API plus the job worker. Jobs live in Postgres, so nothing is lost on restart. To run the
   worker separately, add a second service with the same image and env and
   `command: ["worker"]`, and give the first one `command: ["api"]`. The
   image's health check probes the API, which a worker does not serve, so
   give the worker its own: `healthcheck: { test: ["CMD", "/app/campbooks-server",
   "healthcheck", "--worker"] }` (the image has no shell, and `/app` is not on
   `PATH`). A worker answers `GET /healthz` on
   `CAMPBOOKS_JOBS__HEALTH_ADDR` (default `127.0.0.1:8081`, inside the
   container only): 200 while its job loop runs and reaches the database
   (checked every 10 s), 503 after a minute without. Worker settings are the
   `CAMPBOOKS_JOBS__*` block of `.env.example`.

8. Files (message bodies, attachments, documents) are stored outside Postgres,
   in the `blobs-data` volume by default. See "Blob storage" below to use an
   S3-compatible bucket instead.
9. Workspaces: the first person to sign in creates the first workspace and
   invites the others from Settings → Members. Who else may create a
   workspace is `CAMPBOOKS_SIGNUP__POLICY` (`open` by default; `invite_only`,
   `approval`, `code`; administrators decide `approval` requests and mint single-use `code`
   codes in Admin → Signups, see "Administrators"). Invitations are not emailed yet: the inviter gets the
   link once, right after inviting, to send on; it is built from
   `CAMPBOOKS_PUBLIC_URL`, so set that to the address people use. Campbooks'
   own sign-in knows every scope, so there is nothing to register.
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
-- Only if the migration user is neither a superuser nor has CREATEROLE:
CREATE ROLE campbooks_definer NOLOGIN NOINHERIT;
GRANT campbooks_definer TO <migration user>;
```

`campbooks_definer` owns the database functions that run with their owner's
rights (`SECURITY DEFINER`), so they never run as the migration login. It
cannot log in, holds only the few table privileges those functions need, and
must not be granted to the runtime login or to `campbooks_app`: the server
refuses to start when the role it queries as holds it, and the migrations
refuse a `campbooks_app` that does. The migration user must hold it with
`INHERIT` (the default) so later migrations can replace those functions. The
migrations create it when the migration user may create roles; otherwise they
stop, naming the statements above, until an administrator has run them.

Then set `CAMPBOOKS_DATABASE_URL=postgres://campbooks_runtime:<password>@host/db`
(or omit the password and point `CAMPBOOKS_DATABASE_PASSWORD_FILE` at a file
holding it) and `CAMPBOOKS_DATABASE_MIGRATION_URL` to the owner's URL. On
managed services the "owner" is the user that created the database; do not
reuse it as the runtime login.

**Startup checks.** The server refuses to start (and says why) when the role
it queries as is a superuser, has `BYPASSRLS`, is a member of such a role,
owns tables with row-level security, or is a member of `campbooks_definer`;
and, when a migration login is configured, when the runtime login could still
switch to a privileged role. Only the old single-URL setup (a privileged login
that `SET ROLE campbooks_app`) keeps working, with a warning to split the
logins. If you cannot yet, set `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS=true` to
start anyway; workspace isolation then rests on the queries alone.

**What the separation protects, and when.** With a dedicated runtime login
(the bundled setup, and the one above), no request or job can read, reset or
rewrite the rate-limit counters, the idempotency keys of another caller, or
the web sign-in's tables (sessions and their sealed tokens): it reaches them
only through the database functions. That holds against SQL running as the
runtime role, injected or mistaken. In the single-URL setup, or when the
server runs as the owner login, a query able to `RESET ROLE` gets the owner's
rights back, and with them every table, as it escapes row-level security.

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
- **Jev, a hosted decision model, instead** (ADR 0018). Jev speaks Laya's protocol and
  is served through OpenRouter at another path: set `CAMPBOOKS_AI__DECIDE__PROTOCOL=laya`,
  `CAMPBOOKS_AI__DECIDE__BASE_URL=https://openrouter.ai/api/alpha`,
  `CAMPBOOKS_AI__DECIDE__PATH=decisions`, `CAMPBOOKS_AI__DECIDE__MODEL=typesafe/jev-1.13`
  and your OpenRouter key as `CAMPBOOKS_AI__DECIDE__API_KEY`. It is closed and hosted in
  the US: every inbound email's sender, recipients, subject, list headers, attachment
  names and first 1,500 characters go to OpenRouter and TypeSafe. Campbooks refuses to
  send them until you opt in with `CAMPBOOKS_AI__DECIDE__EXTERNAL_CONTENT=allow`; read
  their region, retention and processing terms first. The shipped defaults stay
  open-weight.
- **What runs without a model.** Understanding what an email says is the model's job
  (ADR 0018). With no `decide` role, or one failing its answers check, Campbooks reads
  headers and addresses only: list mail, automatic mail and no-reply senders need no
  reply, mail stays in Primary, and no need is raised.
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
| Postgres | `postgres-data` volume (use `pg_dump`) | Everything except files and the instance secret key: accounts, sign-in keys, grants |
| Files | `blobs-data` volume, or your bucket | Raw messages, attachments, documents |
| Instance secret key | `server-secrets` volume | `CAMPBOOKS_SECRET_KEY`, which opens the sign-in keys and stored grants in Postgres |

Restore Postgres and the files from the same point in time. A file that is missing from
the store while Postgres still lists it cannot be read, so take the database dump first,
then copy the files: files uploaded in between are only harmless extras. For a bucket,
turn on versioning (and keep old versions for a few days) instead of copying.

For the local volume, for example:

```sh
docker run --rm -v campbooks_blobs-data:/data -v "$PWD":/backup busybox:1.37.0-musl \
  tar czf /backup/blobs-data.tgz -C /data .
```

### After restoring a backup: purge deleted workspaces again

A restore brings every workspace back as it was when the backup was taken. That
includes the ones deleted since, whether already purged or still in their 30-day grace
period, with their members, their mail and their files. Delete each of them again before
anyone signs in. Keep server logs at least as long as your oldest backup, since the logs
are how you find them.

1. **List them.** Search your logs from the backup's time onwards for two messages:
   - `workspace purged`: the workspace was purged after the backup. The line holds the
     workspace's id and the time, nothing else.
   - `workspace deleted`: the workspace was deleted after the backup and may not have
     been purged yet. Its line also holds the `purge_after` it was given.
2. **Purge each again** from the restored database, giving the id twice as
   confirmation:

   ```sh
   docker compose exec server campbooks-server workspaces purge-again <id> --confirm <id>
   ```

   - **If it was already deleted in the backup,** its purge is queued again, or, if a
     purge is queued already, the command changes nothing and says when that purge runs.
   - **If it was still live in the backup** (deleted after the backup was taken), the
     command refuses at first. It prints only that the workspace is live, how many
     members it has and when it was created, so a mistyped id costs nothing. Check those,
     then run it again with `--not-deleted-in-backup`:

     ```sh
     docker compose exec server campbooks-server workspaces purge-again <id> --confirm <id> \
       --not-deleted-in-backup [--purge-after <purge_after from its "workspace deleted" line>]
     ```

     The workspace is hidden at once, and its mailbox connections and app clients are
     revoked at once. Everything else is purged at its purge time: an hour from now, or
     the `--purge-after` you give, which cannot be sooner. Until then, a workspace deleted
     by mistake can still be brought back by restoring again, though its mailboxes then
     need reconnecting.
   - **If the command says the workspace has no owner,** it changes nothing. Every
     workspace keeps an owner, so this means the restored data is not what you expect:
     do not edit the database by hand. Check that you restored the right backup, and if
     it persists, open an issue with the command's output (it holds only ids, counts and
     dates).
   - **An id with no workspace** is reported as such and changes nothing.

   Running the command twice queues a single purge. It logs one line, with the
   workspace's id only, when it changes something.

The server also records each purge in the database: a tombstone, so data that arrives
for a purged workspace later is deleted by the hourly sweep. Those records are restored
to the backup's moment along with everything else, which is why the log lines, not the
database, tell you which workspaces went after it. There is no API for any of this; only
the command does it.

## Monitoring

Out of the box Campbooks exports nothing and opens no extra port. It writes JSON logs to
stdout (`docker compose logs server`); every line inside a request or a job carries its
`trace_id`, and each request ends with one `request finished` line giving the route, the
status and the latency (`latency_ms`), with the request's ids in its `span`: method,
`http.route`, status, `request_id`, `trace_id`, and when known `workspace.id`,
`principal.kind` and `error.type`. Never the path or query string. A web app call through
the BFF is logged under the API route it ran (`campbooks.via=bff`), not `/bff/api/{*path}`.
`/api/v1/system/health` and `/api/v1/system/version` work as before.

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

**Error reports.** Point the server at a Sentry-compatible error tracker (GlitchTip,
Sentry) with the DSN of a project made for it:

```sh
CAMPBOOKS_TELEMETRY__ERROR_DSN=https://<key>@errors.example.com/<project>   # secret
CAMPBOOKS_TELEMETRY__ENVIRONMENT=production                               # or staging, ...
```

Each internal error (a 500), job that failed for good and panic becomes one report, filed
under the release (the image's `GIT_SHA`) and the environment. A report holds the error's
kind (the head of its message, with quotes, addresses, links and tokens masked), the ids
of the request or job it happened in (request, trace, workspace, job), the route template
or job kind and the file and line it was logged at: never the request, the person, a
header, a cookie or what the error was about. Check the setup with

```sh
docker compose exec server campbooks-server telemetry test-error
```

which sends one invented report and prints its id.

**The app's own errors and timings.** The app reports the errors it did not handle and
its web vitals to its server, which scrubs them and forwards them to a
[Grafana Faro](https://grafana.com/oss/faro/) receiver (Grafana Alloy's `faro.receiver`):

```sh
CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL=http://alloy:12347/collect
CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_KEY=<the receiver's api_key>        # secret
```

The app never talks to the receiver itself, so it needs no CORS or CSP change and the
receiver need not face the internet. What is forwarded: the route template shown
(`/w/$workspaceId/inbox`, never the address), a random id for that run of the app, the
browser's name and platform, and for an error its class, the head of its message and the
file names and positions of its stack. Without a receiver the app is told to stop and
sends nothing more.

**Usage analytics.** To see which screens people use, point the server at a
[Matomo](https://matomo.org) site:

```sh
CAMPBOOKS_ANALYTICS__MATOMO_URL=https://analytics.example.com
CAMPBOOKS_ANALYTICS__MATOMO_SITE_ID=1
CAMPBOOKS_ANALYTICS__MATOMO_VISITOR_KEY=<openssl rand -base64 32>     # secret
```

Off unless all three are set. The app loads no analytics script: after each navigation
it tells its own server the screen's route template (`/w/:workspaceId/inbox`, never the
address, an id, a search or a title), and nothing at all while the browser sends Do Not
Track or Global Privacy Control. The server checks the template and sends it to Matomo's
tracking API in the background, with the browser's user agent, the language when
Campbooks ships it and a visitor id that is an HMAC of the account and the UTC day under
the visitor key: daily unique visitors per screen, with no way to follow a person across
days. The id goes as Matomo's forced visitor id (`cid`), which needs no `token_auth`
and makes Matomo tell visits apart by that id alone, although they all come from the
server's address; no Matomo setting has to change. No IP, user id, workspace or custom
dimension is sent. A slow or unreachable Matomo costs dropped views
(`campbooks_screen_views_total{outcome="failed"}`), never a slower app.

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
| `campbooks_connector_syncs_total` | Sync runs of connected accounts | `provider`, `outcome` (`ok`, `throttled`, `refused`, `failed`) |
| `campbooks_connector_overdue_streams` | Streams due for more than 5 minutes that no run picked up | |
| `campbooks_errors_reported_total` | Error reports sent to the tracker | `source` |
| `campbooks_client_telemetry_items_total` | The app's errors, timings and events | `kind`, `outcome` (`forwarded`, `dropped`) |
| `campbooks_screen_views_total` | Screen views relayed to usage analytics | `outcome` (`forwarded`, `failed`, `dropped`: the queue was full) |
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
or email addresses, and metrics never carry ids (workspace ids appear on spans and error
reports only). Tests in the server enforce this for logs, spans, error reports and the
app's forwarded telemetry on every release.

## Outgoing mail

Campbooks sends one kind of mail of its own today: when a way to sign in (a provider
account, a passkey) is added to an account, a notice goes to the account's address and,
for a new provider account, to that account's address too, in the person's language. It
names what was added and when, links only to this instance, and carries no images or
tracking. Without a mail server nothing is sent; the notice is shown in the app (Settings
→ Ways to sign in highlights what is new) and kept in the account's history either way.

Point the server at any SMTP server that accepts mail from it, over TLS:

```sh
CAMPBOOKS_MAIL__SMTP_URL=smtps://user:password@smtp.example.com                      # implicit TLS, port 465
# CAMPBOOKS_MAIL__SMTP_URL=smtp://user:password@smtp.example.com:587?tls=required    # or STARTTLS
CAMPBOOKS_MAIL__FROM=Campbooks <no-reply@example.com>
```

Set both or neither: one without the other stops the server at startup, naming the
setting. TLS is required, so the password never crosses the network in clear: plain
`smtp://`, or `tls=opportunistic` (which anyone on the path can strip), is refused at
startup. The one exception is a relay on the same machine (`smtp://127.0.0.1:25`,
`::1` or `localhost`) given no user or password. The URL holds the password, so it is a secret: the server never logs it or
repeats it in an error, and `CAMPBOOKS_MAIL__SMTP_URL_FILE` reads it from a file instead
(a Docker secret, for example). Mail is sent by the worker and retried when the server is
unreachable; a notice is never sent twice.

## Web sign-in

The server signs browsers in itself (a backend-for-frontend, ADR 0004): open the app, and
if you are not signed in it sends you to Campbooks' own sign-in and back. The browser only
ever gets one `HttpOnly`, `SameSite=Strict` session cookie; the tokens stay on the server,
encrypted, in Postgres (schema `bff`).

Sessions end after 48 hours unused or 30 days at most, or when you sign out. The cookie is
`Secure` and `__Host-`-prefixed when `CAMPBOOKS_PUBLIC_URL` is `https://`; over plain
`http://` (fine on `localhost` only) it is neither, and the server logs a warning.

The `server-secrets` volume holds one file, `secret-key`: `CAMPBOOKS_SECRET_KEY`, which
encrypts the web sessions' tokens, connected accounts' credentials and the sign-in signing
keys, and signs every send and every change Campbooks makes at a mail provider (so rows
written straight into the database never send mail). The server and the worker must share
it: without it, or with a different one, nothing is sent. It is generated on first boot by
`server-init` and mounted read-only into the server (through group 10001). Back it up with
the database: without it the stored keys and grants cannot be opened.

- **Instance secret key:** do not replace it casually; the server refuses to start when it
  cannot open the stored signing keys (see Built-in sign-in). To change it without signing
  anyone out, rotate it (see "Rotating the instance secret key" below). To start over on purpose,
  delete the rows of `identity.signing_keys`, then
  `docker compose run --rm server-init rm /secrets/secret-key` and
  `docker compose up -d && docker compose restart server`. Everyone has to sign in again,
  and connected accounts show "needs reauthorisation" until their member connects them
  again. (Without compose, set `CAMPBOOKS_SECRET_KEY` to `openssl rand -base64 32`.)

Settings (all optional; defaults shown in `.env.example`) are the `CAMPBOOKS_BFF__*` block.

## Rotating the instance secret key

`CAMPBOOKS_SECRET_KEY` seals the web sessions' tokens, connected accounts' credentials and
the sign-in signing keys, and signs sends and changes at mail providers. To replace it
without signing anyone out or disconnecting any mailbox, run the old key beside the new
one for a while as `CAMPBOOKS_SECRET_KEY_PREVIOUS_FILE` (or `CAMPBOOKS_SECRET_KEY_PREVIOUS`).

- **What the previous key does:**
  - it only opens and verifies what was sealed or signed before the rotation;
  - everything new is sealed, signed and derived with the new key;
  - what it opens is sealed again with the new key as it is used: signing keys at once,
    connected accounts within the hour, web sessions (all their tokens) at their next
    token refresh (minutes of use);
  - what it signed is signed again with the new key in the background: taught rules,
    what each rule changed (so it can still be put back), rule runs, change requests,
    scheduled sends, and changes still waiting to be made at a mail provider (outbound
    changes). At start-up and every 15 minutes, in every workspace, two jobs re-tag what
    still verifies only under the old key. Each run logs, per workspace, either
    `binding re-tag: nothing in this workspace is bound under the previous key any more`
    and `binding re-tag: no outbound change in this workspace is bound under the previous
    key any more`, or a warning with how many rows are `left` for the next run.
- **What it does not cover:** the rate-limit counters are keyed by the current key only,
  so they start afresh, which is harmless.
- **Errors:** the server refuses to start when the previous key is set without a current
  one, or equals it. Neither key is ever logged.

With compose:

```sh
# 1. Move the key to secret-key-previous and generate a new one.
docker compose run --rm server-init sh /server/init.sh rotate
# 2. Tell the server about the previous key (compose.override.yaml):
#      services:
#        server:
#          environment:
#            CAMPBOOKS_SECRET_KEY_PREVIOUS_FILE: /run/campbooks/secret-key-previous
docker compose up -d && docker compose restart server
# 3. Before forgetting the old key: the background re-tag has finished. Every workspace
#    has logged both "... bound under the previous key any more" lines (taught rules and
#    sends; outbound changes) since the restart, and no "binding re-tag" warning came
#    after them:
docker compose logs server | grep 'binding re-tag'
# 4. After 30 days (the longest web session; held sign-up grants last 7), remove the
#    variable, then delete the old key:
docker compose run --rm server-init sh /server/init.sh forget-previous
docker compose up -d && docker compose restart server
```

Without compose, set `CAMPBOOKS_SECRET_KEY` to the new key (`openssl rand -base64 32`) and
`CAMPBOOKS_SECRET_KEY_PREVIOUS` to the old one, on every process (API and worker) at once;
remove the previous key after 30 days, once the logs show the re-tag finished (step 3).

- **Before removing the previous key:** wait for step 3. A rule still signed with the old
  key would need confirming again, and its changes could not be put back. An outbound
  change still waiting under the old key (its mailbox paused, or the provider down
  throughout) would be skipped, and its message read again from the provider. A web session
  unused since the rotation (and so not refreshed) ends at its next use, and its member
  signs in again. A connection whose credentials were not used since needs connecting
  again. With mail syncing, connections are used every few minutes.
- **If the old key leaked, do not set it as the previous key.** Whoever holds it could still
  open and forge what it protects for as long as it is accepted. Start over instead (see
  "Instance secret key" above): everyone signs in again, and connected accounts are
  connected again.

## Built-in sign-in (Campbooks as its own identity provider)

Campbooks is its own identity provider (ADR 0014): it signs people in with Google,
Microsoft, Zoho or a passkey and issues every token itself, with no separate service to
run. It runs when `CAMPBOOKS_AUTH__ISSUER` is **unset** (or names `CAMPBOOKS_PUBLIC_URL`
itself) and `CAMPBOOKS_SECRET_KEY` is set; the shipped `compose.yaml` does both.

Pointing `CAMPBOOKS_AUTH__ISSUER` at an outside OpenID Connect issuer is still possible
for an install that needs one (the API then trusts that issuer's tokens, set
`CAMPBOOKS_AUTH__JWKS_URL` and `CAMPBOOKS_AUTH__AUDIENCE`, and the web app signs in
through it with `CAMPBOOKS_BFF__CLIENT_SECRET[_FILE]`), but Campbooks does not provision
or document any particular issuer, and its own sign-in, ways in, apps and mailbox
connection are off while you do.

With the built-in issuer:

- The issuer is `CAMPBOOKS_PUBLIC_URL` (no path). Its metadata is at
  `/.well-known/openid-configuration` (and `/.well-known/oauth-authorization-server`), its
  keys at `/oauth2/jwks`, and its endpoints under `/oauth2/*`. The web app signs in through
  it with no client secret to provision.
- It signs with Ed25519 keys it generates on first start and rotates every 30 days,
  stored in Postgres (schema `identity`) encrypted with `CAMPBOOKS_SECRET_KEY`. A server
  whose secret key does not open them refuses to start ("CAMPBOOKS_SECRET_KEY does not
  match the stored signing keys"): restore the right key. To replace the secret key on
  purpose, also delete the rows of `identity.signing_keys`; everyone signs in again.
- The public URL must be `https://` (plain `http://` only with `CAMPBOOKS_ENV=development`),
  so the sign-in cookies are always `Secure`.
- New accounts follow `CAMPBOOKS_SIGNUP__POLICY`: on `invite_only`, only people whose
  Google address has a pending invitation get an account.
- **Only your own people.** `CAMPBOOKS_SIGNUP__ALLOWED` is a comma-separated list of
  addresses (`ana@example.com`) and domains (`example.com`, which matches
  `*@example.com` exactly, not subdomains), case-insensitive. Unset or empty, nothing is
  restricted. When set, every way in checks it on top of the policy (an invitation does
  not bypass it): a provider sign-in needs a provider-verified address on the list, for
  new and existing accounts alike; a new account is checked again when it is created;
  a way in added later must be on it; a passkey signs in only an account that has a
  listed, provider-verified address and cannot create an account; and token refresh is
  refused for an account that no longer qualifies, so removing someone cuts their access
  at their next refresh (about every ten minutes) and a restart is enough to apply a
  change. Refused people see "This Campbooks isn't open to this address" and nothing is
  kept; the message never says whether an account exists. A malformed entry stops startup
  (the error names the entry's position, not its text).
- **Client secrets are files.** `compose.yaml` mounts `deploy/secrets/` read-only into the
  server at `/run/secrets/campbooks`: put each provider's secret in a file there
  (`google-client-secret`, `microsoft-client-secret`, `zoho-client-secret`, readable by uid
  10001: `sudo chown 10001:10001` and `chmod 0400`; Docker Desktop needs neither) and set
  `CAMPBOOKS_<PROVIDER>__CLIENT_ID` in `.env` and
  `CAMPBOOKS_<PROVIDER>__CLIENT_SECRET_FILE=/run/secrets/campbooks/<file>`.
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

## API rate limits

Every API call made with a token is counted against a limit for whoever makes it, so a
stolen token, a buggy app or a looping automation rule cannot hammer the instance or send
mail as fast as the provider allows. Nothing to configure: the defaults are far above what
a person does in the app, even in a fast keyboard session with live updates.

| Class | Default | Counted per |
|---|---|---|
| Reads | 600 a minute | person and app (an app on its own: its client id) |
| Writes | 120 a minute | person and app |
| Mail sends | 30 per 10 minutes | person, across all their apps |
| Admin API | 120 a minute | administrator and app |

- A person's apps (the web app, the desktop app, a third-party app they allowed) each
  get their own budget, so a misbehaving app never locks them out of the web app.
  Automation rules and the assistant acting for a person are counted apart from the
  person's apps, at the same limits; their sends count against the person's.
- Reads and writes allow a burst of the whole count, then one every window / count.
  Over a limit the answer is 429 `rate_limited` with `Retry-After` (seconds).
- Reads, writes and admin calls are counted in each server process's memory, so with
  several API processes a caller may get that many times the budget, and a restart
  starts them afresh. Sends are counted in Postgres, shared by every process.
- Change them with `CAMPBOOKS_RATE_LIMITS__READS`, `__WRITES`, `__SENDS` and `__ADMIN`,
  and each class's `…_WINDOW_SECONDS` (1 second to 1 day); `0` turns a class off and
  `CAMPBOOKS_RATE_LIMITS__ENABLED=false` turns them all off. See `deploy/.env.example`.
- A caller that reaches a limit is logged once (`a caller reached an API rate limit`,
  with the class, the kind of caller and the operation, never who), and every refusal
  counts in the metric `campbooks.rate_limit.refusals`.

The sign-in, token, registration and search limits per address are separate and
unchanged.

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
share it, or with an outside identity provider, pass `--account <id>` instead: the id is the `sub` that
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
- The admin API asks for a recent sign-in: more than an hour after signing in, the app asks
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

Connecting needs the built-in sign-in: an install that sets `CAMPBOOKS_AUTH__ISSUER` to an
outside issuer cannot connect mail. `connectors.start_connection` answers with the
built-in sign-in's URL, which comes back to the app with `connected=<connection id>`.

### Developing the sign-in locally

The API runs on the host in development (`compose.dev.yaml` only starts Postgres), and the
built-in sign-in needs no extra service: run the server with `CAMPBOOKS_ENV=development`,
`CAMPBOOKS_SECRET_KEY` (any `openssl rand -base64 32`) and a `CAMPBOOKS_PUBLIC_URL` of where
you open the app (`http://localhost:8080` for `cargo run`, `http://localhost:5173` for the
Vite dev server), with `CAMPBOOKS_GOOGLE__CLIENT_ID` and `CAMPBOOKS_GOOGLE__CLIENT_SECRET`
from a Google OAuth client that allows `${CAMPBOOKS_PUBLIC_URL}/auth/callback/google`, or
sign in with a passkey. `clients/README.md` has the full recipe.

## Upgrading from the Rauthy sign-in

Releases before this one shipped Rauthy (the `idp`, `idp-init` and `idp-provision`
services and the `idp-data` volume). Copy the new `compose.yaml` over yours and run
`docker compose up -d --remove-orphans`: the three services are removed and the server
signs in through Campbooks' own issuer. Nothing else needs doing, and nothing breaks if you
leave the old services running, but they are no longer used. The `idp-data` volume is
kept; remove it yourself once you are sure you do not need it
(`docker volume rm <project>_idp-data`).

Accounts that existed in Rauthy are not carried over: people sign in again with Google,
Microsoft, Zoho or a passkey and get a new Campbooks account. Move the old `CAMPBOOKS_IDP_*`
provider settings to the `CAMPBOOKS_GOOGLE__*`, `CAMPBOOKS_MICROSOFT__*` and
`CAMPBOOKS_ZOHO__*` ones (see Built-in sign-in), and add
`${CAMPBOOKS_PUBLIC_URL}/auth/callback/<provider>` to each provider's OAuth app.
