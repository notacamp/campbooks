# Changelog

All notable changes to Campbooks are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
See [CONTRIBUTING.md](CONTRIBUTING.md#versioning--releases) for what counts as a
major, minor, or patch change here.

<!--
  Add your change under [Unreleased], in the matching group. Maintainers move it
  under a new version heading at release time.
  Groups, in order: Added · Changed · Deprecated · Removed · Fixed · Security.
  Pre-1.0: flag breaking changes under "Changed" with a ⚠️.
-->

## [Unreleased]

### Added

- Configurable folder icons — the inbox folder bar now renders an icon on every folder chip, and custom folders can be given an icon from a picker when created.
- Prometheus metrics at an internal `/metrics` endpoint ([yabeda](https://github.com/yabeda-rb/yabeda)):
  HTTP request rate / error rate / latency (RED), background-job execution counts
  and duration, and a domain-action counter sourced from the Events bus. Meant to
  be scraped over a private network and visualized in Grafana. Multi-process safe
  via the Prometheus client's `DirectFileStore` (`PROMETHEUS_MULTIPROC_DIR`), with
  the Solid Queue worker exposing its own metrics server on `:9394`. See
  [docs/observability.md](docs/observability.md).
- Official production container images, published to the GitHub Container
  Registry (`ghcr.io/notacamp/campbooks`) when a release is published. Multi-arch
  (`linux/amd64` + `linux/arm64`) and tagged by semantic version (`1.2.3`, `1.2`)
  plus `latest` for the newest stable release, so self-hosters can pull a prebuilt
  image — on x86 or ARM — instead of building from source. Images are
  tagged by semantic version (`1.2.3`, `1.2`) plus `latest` for the newest stable
  release, so self-hosters can pull a prebuilt image instead of building from
  source. The full test suite re-runs as a gate before any image is pushed.
- A **Select mode** for the inbox — a toolbar toggle that turns the thread list
  into a batch organizer: persistent checkboxes on every row *and* every date /
  Priority section divider (so multi-select works on touch, not just on hover),
  tap-a-row-to-select, a select-all-per-section checkbox with an indeterminate
  state when only some of a section's threads are picked, and the docked
  bulk-action bar (archive, tag, snooze, move, delete, …). Toggle off or press
  Esc to exit.
- A machine-readable [OpenAPI 3 specification](openapi.yaml) for the public REST
  API, plus an expanded reference ([docs/api.md](docs/api.md)) with per-resource
  response examples, Python/JavaScript samples, and a complete error-code table.
  Settings → API access now links to the documentation (the URL is configurable
  via `API_DOCS_URL`).

### Fixed

- The `emails:write` API scope description shown in Settings → API access no
  longer overstates what it grants — it marks emails read/unread (it does not
  archive, snooze, or tag).

### Fixed

- Drag-and-drop and tap-to-move no longer offer Sent or Drafts as destinations (moving received mail into outbound/compose folders made no sense).

## [0.1.0] - 2026-06-25

### Added

- Initial public, source-available release of Campbooks.

[Unreleased]: https://github.com/notacamp/campbooks/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/notacamp/campbooks/releases/tag/v0.1.0
