# Campbooks

**The email client that declutters your work** — an AI-native inbox that sorts itself,
reimagined so it feels nothing like the email you're used to. Built for young professionals
and small-business owners buried in email and paperwork.

Campbooks reads your email and attachments, files the invoices, receipts, and contracts on
its own, and hands you one short list of what actually needs you — so your attention goes to
decisions, not drudgery. Its AI assistant, **Scout**, briefs you each morning, answers
questions about your inbox and documents, and drafts replies you can approve.

Source-available under the Sustainable Use License — run it on our hosted cloud or self-host it on your own
server.

## Stack

Rails 8.1 · PostgreSQL · Tailwind CSS 4 · Hotwire · Solid Queue · Phlex components.

## Self-hosting (Docker)

Run your own instance in a few minutes — Postgres, the web app, and the job
worker, all from one Compose file:

```bash
cp .env.example .env
bin/generate-secrets          # generates the required secrets into .env
docker compose up -d --build
```

Then open <http://localhost:3000> and register the first account. The app runs
without any external API keys; AI features and mailbox connections light up as
you add credentials. Full guide — including every environment variable, each
integration's setup, and production/TLS — is in
[`docs/self-hosting.md`](docs/self-hosting.md).

## Getting started (development)

```bash
bin/setup                     # install dependencies and prepare the database
bin/dev                       # web + Tailwind + worker (Foreman), on :3000
```

`bin/dev` also starts the dev dependencies in `docker-compose.dev.yml`.

Seed login (created by `db/seeds.rb`): `admin@example.com` / `changeme123`.
The login form is at `/session/new`.

## Documentation

- **Positioning & voice:** [`docs/messaging.md`](docs/messaging.md) — how we talk about the product
- **Architecture & conventions:** [`CLAUDE.md`](CLAUDE.md)
- **Domain glossary:** [`CONTEXT.md`](CONTEXT.md)
- **Product vision:** [`PRODUCT.md`](PRODUCT.md), [`DESIGN.md`](DESIGN.md)
- **Feature & deployment guides:** [`docs/`](docs/)

## Contributing

Contributions are welcome — from human developers and AI agents alike. Start with
[`CONTRIBUTING.md`](CONTRIBUTING.md): branch off `main`, open a PR with a
[Conventional Commits](https://www.conventionalcommits.org/) title, and make CI
green. Please also read our [Code of Conduct](CODE_OF_CONDUCT.md). Found a
security issue? Follow [`SECURITY.md`](SECURITY.md) — never a public issue.

Changes are tracked in [`CHANGELOG.md`](CHANGELOG.md) and released as
[semver](https://semver.org) `vX.Y.Z` tags.

## License

[Sustainable Use License](LICENSE).
