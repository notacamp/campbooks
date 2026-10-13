# Design

## Context

`image.yml` is the one reusable recipe that builds, publishes, scans and
attests the server image; `publish-next.yml` (every green push to `next`) and
`release-next.yml` (versioned releases) call it. `deploy/laya/Dockerfile`
already exists (ADR 0009, change `decide-role-laya`): Python slim, CPU-only
PyTorch, `laya[serve]` pinned, non-root, multi-arch safe, the checkpoint
downloaded into a volume on first start. It was only ever built by
`docker compose build`.

## Decisions

### One recipe, two calls

`image.yml` gains two inputs, `image` (package name under
`ghcr.io/<owner>/`, default `campbooks`) and `context` (default `.`). The
callers add a `laya-image` job calling it with `image: campbooks-laya` and
`context: deploy/laya` and the same `tags` block as the server job. Inside
the recipe the digest artifacts and registry build cache are already keyed by
the image name (the cache ref lives on each package), so the two calls never
collide. `GIT_SHA` is passed only to the server build.

### Same tags as the server image

A deploy pins one tag (`CAMPBOOKS_IMAGE_TAG`) for both services, and a
rollback to an older tag brings back the Laya image that shipped with it.
For that to hold, every tag must exist for both images: the staging dispatch
(`deploy-staging`) and the GitHub release (`release`) `need` both image jobs.
The cost is that a Trivy finding in Laya's Python dependencies blocks a
server publish too; the fix is the same as for the server (bump the pin), and
an operator can still pin Laya to an older tag separately.

### Gates on PRs

CI gets a `laya-image` job: native amd64 and arm64 builds with `load: true`
and the same Trivy gate, not pushed. The image is heavy (PyTorch), so the job
builds only when `deploy/laya`, `ci.yml` or `image.yml` changed against the
PR base (or the previous `next` commit); otherwise it passes without building.
It is not a required check.

### Checkpoint stays out of the image

Kept from `decide-role-laya`: the image holds code only and downloads the
checkpoint (~680 MB) at the revision the `laya` package lists as reviewed
into the `laya-models` volume on first start.

### Visibility

The package is created on the first publish by `GITHUB_TOKEN`, linked to this
repository by the `org.opencontainers.image.source` label. Like the server
image it must be public so self-hosters and our hosts pull it without
registry credentials; an org owner sets that once in the package settings.

## Alternatives considered

1. **A separate `laya-image.yml` workflow with its own tags and schedule.**
   Builds only when `deploy/laya` changes, so no PyTorch rebuild per push.
   Rejected: a second recipe to keep in step with the server's (scan,
   attestation, pinning), and a second tag scheme deployments must track.
   With registry cache an unchanged Laya build is a cache hit, so the saving
   is small.
2. **Bake the checkpoint into the image.** First start is instant and needs
   no Hugging Face access. Rejected for now: +680 MB per architecture per tag,
   the model weights' licence and provenance then ride on our image, and a
   model change would need an image release. The volume download happens
   once per host.
3. **Keep building on the host (status quo).** No pipeline work. Rejected:
   production hosts must not build, and the image would stay unscanned.

None of this copies a pattern from the Rails app on `main`; it extends the
`next` image recipe.

## Data ownership, events, scopes, AI

No module data, events, endpoints or scopes. AI role: `decide`; the image is
optional, and when the role is not configured the conversations decide pass
is skipped and the rules alone sort mail (unchanged).

## Risks

- Trivy may flag fixable HIGH/CRITICAL findings in the Python stack or the
  Debian base; fixed by bumping pins in `deploy/laya`.
- arm64 PyTorch CPU inference speed per message is not measured here.
