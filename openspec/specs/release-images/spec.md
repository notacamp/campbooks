# release-images Specification

## Purpose
Which container images the release pipeline publishes, for which
architectures, under which tags, and what must pass before a tag is deployed
or released.

## Requirements

### Requirement: Published images
The release pipeline SHALL publish two container images from one commit: the
Campbooks server image (`ghcr.io/notacamp/campbooks`, built from the
repository root) and the Laya model server image
(`ghcr.io/notacamp/campbooks-laya`, built from `deploy/laya`). Each SHALL be a
multi-architecture manifest for linux/amd64 and linux/arm64.

#### Scenario: Green push to next
- **WHEN** CI succeeds on a push to `next` at commit `<sha>`
- **THEN** both images are published as `next` and `next-<sha12>`, each with linux/amd64 and linux/arm64 entries

#### Scenario: Versioned release
- **WHEN** a release commit for version `X.Y.Z` passes CI
- **THEN** both images are published as `next-vX.Y.Z` and `next-vX.Y`, and the release notes name both digests

### Requirement: Same tags, gated together
A tag SHALL be dispatched to a deployment or released only once both images
under that tag are published and have passed the vulnerability gate (no
fixable HIGH or CRITICAL findings).

#### Scenario: Laya image fails the scan
- **WHEN** the Laya image has a fixable CRITICAL finding
- **THEN** the staging dispatch (or the release) for that tag does not run

### Requirement: Model weights outside the image
The Laya image SHALL NOT contain model weights; it SHALL download the
checkpoint into its models volume on first start and reuse it afterwards, and
SHALL refuse to start without an API key.

#### Scenario: First start
- **WHEN** the Laya container starts with an empty models volume and an API key
- **THEN** it downloads the checkpoint into the volume before reporting healthy

#### Scenario: No API key
- **WHEN** the Laya container starts with an empty `LAYA_API_KEY`
- **THEN** it exits with a non-zero status and a message naming the setting
