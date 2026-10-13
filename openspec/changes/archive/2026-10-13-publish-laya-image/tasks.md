# Tasks

## 1. Pipeline

- [x] 1.1 Parameterise `image.yml` by package name and build context (digest artifacts keyed by image; `GIT_SHA` only for the server); verify the server calls keep their defaults
- [x] 1.2 Add `laya-image` to `publish-next.yml` and `release-next.yml` with the server's tags; make the staging dispatch and the release wait for it; add the Laya digest to the release notes
- [x] 1.3 Add a CI `laya-image` job (amd64 + arm64, load, Trivy, smoke test: refuses no key, answers timed decide requests) that builds only when its inputs changed; verify it runs green on this PR

## 2. Deploy

- [x] 2.1 Point the compose `laya` service at `ghcr.io/notacamp/campbooks-laya:next`, keeping `build: ./laya` commented; verify with `docker compose --profile ai config -q`
- [x] 2.2 Update `deploy/README.md` (Local AI) and the Dockerfile header

## 3. Wrap-up

- [x] 3.1 CHANGELOG updated
- [x] 3.2 OpenSpec validated and archived; CI green
