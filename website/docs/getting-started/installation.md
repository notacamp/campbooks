---
title: "Installation"
description: "How to install and run Campbooks on your own server"
sidebar_position: 2
---

Install Campbooks on your own server. You'll need Ruby 3.3+, PostgreSQL 16+, and Node.js 18+.

<div class="callout callout-note">
  **New to Rails?** Campbooks is a standard Rails application. If you've deployed Rails before, this will feel familiar. Most steps follow Rails conventions.
</div>

## Prerequisites

- **Ruby** 3.3 or later
- **PostgreSQL** 16 or later
- **Node.js** 18 or later
- **Redis** (for Action Cable, optional — uses Solid Cable by default)
- **OpenSearch** (for full-text search, optional — uses PostgreSQL by default)

## Clone the repository

```bash
git clone https://github.com/notacamp/campbooks.git
cd campbooks
```

## Install dependencies

```bash
bundle install
```

## Set up the database

```bash
bin/rails db:create
bin/rails db:migrate
bin/rails db:seed
```

<div class="callout callout-info">
  **Seed users.** The seed command creates two accounts for testing:
  `admin@example.com` and `partner@example.com`, both with password `changeme123`.
</div>

## Configure environment variables

Copy the example environment file and fill in your values:

```bash
cp .env.example .env
```

**Required:**

| Variable | Purpose |
|----------|---------|
| `DATABASE_URL` | PostgreSQL connection string |
| `ACTIVE_RECORD_PRIMARY_KEY` | Encryption primary key |
| `ACTIVE_RECORD_DETERMINISTIC_KEY` | Encryption deterministic key |
| `ACTIVE_RECORD_KEY_DERIVATION_SALT` | Encryption key derivation salt |

**Optional but recommended:**

| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_API_KEY` | Claude API key for AI features |
| `OPENAI_API_KEY` | OpenAI API key for embeddings |
| `ZOHO_CLIENT_ID` / `ZOHO_CLIENT_SECRET` | Zoho Mail OAuth credentials |
| `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` | Google OAuth credentials |

<div class="callout callout-warning">
  **Encryption keys are required.** Without `ACTIVE_RECORD_PRIMARY_KEY`, `ACTIVE_RECORD_DETERMINISTIC_KEY`, and `ACTIVE_RECORD_KEY_DERIVATION_SALT`, the app will fail to start. Generate them with `bin/rails secret` and use the output for each key.
</div>

Generate encryption keys:

```bash
bin/rails secret
```

## Start the application

```bash
bin/rails server               # Web server on port 3000
bin/rails solid_queue:start    # Background job worker
```

Or with the Procfile:

```bash
bin/dev
```

Open `http://localhost:3000` and sign in with one of the seed users.

## Docker

A Dockerfile is provided for production deployments:

```bash
docker build -t campbooks .
docker run -p 3000:3000 --env-file .env campbooks
```

<div class="callout callout-note">
  **Next step.** See the [Deployment guide](/docs/deployment/overview) for a full production setup with Nginx, SSL, and systemd.
</div>
