# Tasks

## 1. Config and relay

- [x] 1.1 `TrackingConfig::amqp_password_file` and `amqp_connection_url()`: the file's password replaces the URL's, percent-encoded, trailing newline dropped
- [x] 1.2 `TrackingConfig::check`: with the sink `amqp`, a missing or empty file fails, naming the setting only
- [x] 1.3 The relay connects with the resolved URL
- [x] 1.4 Unit tests (resolution, blank path, missing/empty file, the setting loads) and a broker test (the file's password signs in; a wrong one is refused)

## 2. Docs, spec and changelog

- [x] 2.1 `docs/tracking.md`, `deploy/README.md`, `deploy/.env.example`
- [x] 2.2 Spec delta on `tracked-events`, change archived
- [x] 2.3 Changelog entry file
