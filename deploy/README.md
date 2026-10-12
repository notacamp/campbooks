# Self-hosting Campbooks

1. `cp .env.example .env` and set at least `POSTGRES_PASSWORD` and `CAMPBOOKS_PUBLIC_URL`.
2. `docker compose up -d` starts the server and Postgres (with pgvector) on port 8080.
   The server answers both the web app (open `http://localhost:8080/`) and the API
   (`/api/v1/...`) on that one port.
3. Check it: `curl http://localhost:8080/api/v1/system/health` should return `{"status":"ok","database":"ok"}`.
4. Optional local AI: `docker compose --profile ai up -d`, then pull the models once:
   `docker compose exec ollama ollama pull qwen3:8b` (TEI downloads `BAAI/bge-m3` itself).
5. Optional identity provider: `--profile auth` runs Rauthy; it needs a generated
   `rauthy/config.toml` first (see the comment in `compose.yaml`). The API validates
   its access tokens once `CAMPBOOKS_AUTH__ISSUER` (and, inside compose,
   `CAMPBOOKS_AUTH__JWKS_URL`) is set in `.env`; see the "Access tokens" block in
   `.env.example`. Without an issuer, endpoints that require a scope answer 401
   `auth_not_configured`; health and version stay public.
6. Upgrades: `docker compose pull && docker compose up -d`; migrations run on startup.

CPU inference works but is slow. For the `decide` and `complete` roles, use a GPU
host or a hosted provider serving open-weight models (set its URL in `.env`).
