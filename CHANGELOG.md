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
  be scraped over a private network and visualized in Grafana — see
  [docs/observability.md](docs/observability.md).

## [0.1.0] - 2026-06-25

### Added

- Initial public, source-available release of Campbooks.

[Unreleased]: https://github.com/notacamp/campbooks/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/notacamp/campbooks/releases/tag/v0.1.0
