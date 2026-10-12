# Self-hosting Campbooks

1. `cp .env.example .env` and set at least `POSTGRES_PASSWORD` and `CAMPBOOKS_PUBLIC_URL`.
2. `docker compose up -d` starts the API server and Postgres (with pgvector) on port 8080.
3. Check it: `curl http://localhost:8080/api/v1/system/health` should return `{"status":"ok","database":"ok"}`.
4. Optional local AI: `docker compose --profile ai up -d`, then pull the models once:
   `docker compose exec ollama ollama pull qwen3:8b` (TEI downloads `BAAI/bge-m3` itself).
5. Optional identity provider: `--profile auth` runs Rauthy; it needs a generated
   `rauthy/config.toml` first (see the comment in `compose.yaml`). Auth is not wired yet.
6. Upgrades: `docker compose pull && docker compose up -d`; migrations run on startup.

CPU inference works but is slow. For the `decide` and `complete` roles, use a GPU
host or a hosted provider serving open-weight models (set its URL in `.env`).
