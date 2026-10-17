# Tasks

## 1. Server

- [x] 1.1 `telemetry::served_route`: names the outer request's span after the matched API route (`campbooks.via=bff`) and puts it on the response
- [x] 1.2 `observe` counts and logs the request under the served route; the BFF's API and admin routers get the route layer
- [x] 1.3 Tests: the request log through a BFF-shaped router (`tests/request_log.rs`), the metrics through the real BFF (`api_calls_are_counted_under_the_api_route`)

## 2. Docs

- [x] 2.1 Changelog; `deploy/README.md` monitoring notes
