# Proposal

## Why

The server answers its health check only at `GET /api/v1/system/health`. Load
balancers, uptime monitors and the self-hosting docs still probe `/up`, the
path of the previous app. Today `/up` falls through to the web client: a
problem+json 404 for `curl`, the web app for a browser, so a probe of `/up`
reads as a failure or, worse, as a healthy page.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/222

## What Changes

- Modules touched: `system` (exports its health handler) and the server
  (mounts it).
- The server answers `GET /up` (and `HEAD`) with exactly the health check's
  result: 200 `{"status":"ok","database":"ok"}`, or the localized 503
  problem+json when the database does not answer. Same handler, no copy of
  the database check.
- `/up` is uncached (`Cache-Control: no-store`) and never answered by the web
  client.
- It is not an API operation: it is not in the OpenAPI document or the
  operation catalogue.

## Non-goals

- No change to `/api/v1/system/health`, its response or its OpenAPI entry.
- No change to the `healthcheck` subcommand or the container health check.
- No change to the OpenAPI document.
