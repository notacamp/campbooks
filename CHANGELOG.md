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

- Prometheus metrics at an internal `/metrics` endpoint ([yabeda](https://github.com/yabeda-rb/yabeda)):
  HTTP request rate / error rate / latency (RED), background-job execution counts
  and duration, and a domain-action counter sourced from the Events bus. Meant to
  be scraped over a private network and visualized in Grafana. Multi-process safe
  via the Prometheus client's `DirectFileStore` (`PROMETHEUS_MULTIPROC_DIR`), with
  the Solid Queue worker exposing its own metrics server on `:9394`. See
  [docs/observability.md](docs/observability.md).
- Official production container images, published to the GitHub Container
  Registry (`ghcr.io/notacamp/campbooks`) when a release is published. Images are
  tagged by semantic version (`1.2.3`, `1.2`) plus `latest` for the newest stable
  release, so self-hosters can pull a prebuilt image instead of building from
  source. The full test suite re-runs as a gate before any image is pushed.

### Changed

- ⚠️ Several features that aren't production-ready yet now ship **disabled by
  default** and are opt-in via environment flags (all default off, in both cloud
  and self-hosted builds). Set the matching var to `1` to re-enable:
  - **Workflow engine** (`ENABLE_WORKFLOWS`) — the builder UI, navigation/Cmd+K
    entries, controllers, public webhook ingress, public API, and the automatic
    email/event triggers are all gated; when off the UI/API return 404 and no
    workflow fires.
  - **Inbox "Board" (kanban) layout** (`ENABLE_EMAIL_BOARD`) — the inbox view
    switcher offers only Default and List; the board route returns 404.
  - **Microsoft 365** (`ENABLE_MICROSOFT`) — every Microsoft surface, now
    including "Sign in with Microsoft" (previously always shown), is hidden. This
    supersedes the old `ENABLE_MICROSOFT_MAILBOX` flag, which is still honored for
    backward compatibility.

## [0.1.0] - 2026-06-25

### Added

- Initial public, source-available release of Campbooks.

[Unreleased]: https://github.com/notacamp/campbooks/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/notacamp/campbooks/releases/tag/v0.1.0
