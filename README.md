# Campbooks

**An inbox that sorts itself** — an AI-native assistant for email, calendar,
tasks, files and accounting, for young professionals and small-business owners
drowning in email and paperwork. Open source, self-hostable, private by design.

> 🚧 **This is `next`: a from-scratch rewrite** (Rust API + one UI for web,
> desktop and mobile). It is in its foundation phase and not usable yet. The
> current production app (Rails) lives on [`main`](https://github.com/notacamp/campbooks/tree/main).

## What's different in Next

- **Rust API**, a modular monolith of vertical-slice modules — [architecture](docs/architecture.md)
- **One UI everywhere** — React/TypeScript shipped to web, macOS, Windows, Linux, iOS and Android with Tauri 2
- **Standard auth** — OAuth 2.1/OIDC for our apps and yours; tokens never reach browser JavaScript
- **Open-weight AI only** — every model can run on your own hardware; you configure every role
- **Spec-driven** — behaviour is specified in [`openspec/`](openspec/) before it's built

## Quick start (development)

Prerequisites: Rust (pinned by `rust-toolchain.toml`), Node 22 + pnpm, Docker.

```sh
docker compose -f compose.dev.yaml up -d          # Postgres + pgvector on :55432
export DATABASE_URL=postgres://postgres:postgres@localhost:55432/campbooks
cargo run -p campbooks-server                       # API on :8080 (/openapi.json)
pnpm --dir clients install && pnpm --dir clients dev   # web client
```

Self-hosting: see [`deploy/`](deploy/README.md).

## Contributing

Read [`AGENTS.md`](AGENTS.md) (rules for humans and agents alike) and
[`CONTRIBUTING.md`](CONTRIBUTING.md). Decisions are in [`docs/adr/`](docs/adr/).

## License

[Sustainable Use License](LICENSE) — source-available; free to self-host and
modify for your own use.
