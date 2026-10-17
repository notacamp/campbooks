# Tasks

## 1. Server

- [x] 1.1 `campbooks-system` exports the health handler; the server mounts it at `/up`, outside the API router
- [x] 1.2 `/up` is sent `Cache-Control: no-store`
- [x] 1.3 Tests: same answer as the health check, 503 problem+json when the database is down, uncached, not shadowed by the web client, absent from OpenAPI

## 2. Docs

- [x] 2.1 `deploy/README.md` mentions the alias
- [x] 2.2 OpenAPI unchanged (`/up` is not an API operation; no client types to regenerate)
- [x] 2.3 CHANGELOG updated
- [ ] 2.4 Gates green in CI
