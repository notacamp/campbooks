# Proposal

## Why

The `decide` role's default model server, Laya, has no official container
image, so `deploy/compose.yaml` builds one from `deploy/laya` on the operator's
machine. That works for a self-hoster with Docker and time, but not for a
production host that never builds images (our cloud deploys pull tags CI
built, scanned and attested), and nothing in the release pipeline scans what
the `laya` service runs.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/20 (real AI
in production: Laya on the server for `decide`).

## What Changes

- The release pipeline publishes `ghcr.io/notacamp/campbooks-laya` from
  `deploy/laya` next to the server image, through the same reusable recipe
  (`image.yml`, now parameterised by package name and build context):
  linux/amd64 + linux/arm64 built natively, Trivy-gated (HIGH/CRITICAL,
  fixable), SBOM and provenance attested, under exactly the server image's
  tags (`next`, `next-<sha>`, `next-vX.Y.Z`, `next-vX.Y`).
- The staging dispatch and the GitHub release wait for both images, so any
  tag that is deployed or released exists for both.
- CI builds and scans the Laya image on both architectures when `deploy/laya`
  or the image recipe changes (not pushed).
- `deploy/compose.yaml`'s `laya` service uses the published image; `build:
  ./laya` stays as a commented alternative for local development.
- Unchanged: the checkpoint is still downloaded on first start into the
  `laya-models` volume (not baked into the image), and the entrypoint still
  refuses to start without `LAYA_API_KEY`.

## Capabilities

### New Capabilities

- `release-images`: what the release pipeline publishes, for which
  architectures, under which tags, and what gates a publish.

### Modified Capabilities

None.

## Impact

- Workflows: `image.yml`, `publish-next.yml`, `release-next.yml`, `ci.yml`.
- Deploy: `deploy/compose.yaml`, `deploy/README.md`, `deploy/laya/Dockerfile`
  (comment only).
- No module, schema, endpoint, scope or event changes; no Rust code.

## Non-goals

- Baking the checkpoint into the image (see design.md, Alternatives).
- Configuring AI for any particular deployment (the cloud repo does that).
- Publishing images for Ollama or TEI (they have official images).
- Changing Laya's version, its protocol or the gateway.
