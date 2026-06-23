---
title: "Deployment Overview"
description: "How to deploy Campbooks to production"
sidebar_position: 1
---

Deploy Campbooks on your own infrastructure. It's a standard Rails application and can run on any server that supports Ruby and PostgreSQL.

## Deployment options

- **Kamal** — deploy to any VPS with Docker
- **Docker Compose** — run on a single server
- **Heroku / Render** — platform-as-a-service
- **Bare metal** — run directly on a server

## Recommended stack

For a production deployment:

- **Web server**: Puma (bundled with Rails)
- **Background jobs**: Solid Queue (database-backed, no Redis required)
- **Database**: PostgreSQL 16+
- **Storage**: Local disk or S3-compatible (AWS S3, MinIO, Cloudflare R2)
- **Reverse proxy**: Nginx or Caddy
- **SSL**: Let's Encrypt via Caddy or Certbot

## Environment variables

All configuration is via environment variables. Key ones:

| Variable | Required | Purpose |
|----------|----------|---------|
| `DATABASE_URL` | Yes | PostgreSQL connection string |
| `RAILS_ENV` | Yes | Set to `production` |
| `SECRET_KEY_BASE` | Yes | Rails secret key |
| `ACTIVE_RECORD_PRIMARY_KEY` | Yes | Encryption key |
| `ACTIVE_RECORD_DETERMINISTIC_KEY` | Yes | Encryption key |
| `ACTIVE_RECORD_KEY_DERIVATION_SALT` | Yes | Encryption salt |

Generate `SECRET_KEY_BASE`:

```bash
bin/rails secret
```

Generate encryption keys:

```bash
bin/rails db:encryption:init
```

## Precompiling assets

Before deploying, precompile assets:

```bash
RAILS_ENV=production bin/rails assets:precompile
```

## Running in production

```bash
RAILS_ENV=production bin/rails server
RAILS_ENV=production bin/rails solid_queue:start
```

Or use the Procfile with a process manager like systemd or supervisor.

## Health check

Campbooks includes a health check endpoint at `/up`. Use this for monitoring and load balancer health checks.
