# 0001 — Spec-driven development with OpenSpec

**Status:** Accepted · 2026-10-12

## Context
The previous Rails app grew by accretion: features were built straight from
ideas, the behaviour of the product lived only in code, and much of the work is
done with AI agents, which drift without a written target.

## Decision
Use [OpenSpec](https://github.com/Fission-AI/OpenSpec) (pinned `1.13.2`):
- `openspec/specs/<domain>/spec.md` is the durable baseline of how the product
  behaves, one directory per domain module.
- Every behaviour change is an `openspec/changes/<name>/` with a proposal,
  design, tasks and spec deltas, approved before implementation and archived
  into the baseline after merge.
- One change = one branch = one PR = one tracking issue (recorded in the
  change's `.openspec.yaml`).
- CI runs `openspec validate --all --strict`.

## Consequences
- Specs are written fresh. The Rails app is a reference for what users needed,
  not a source to port.
- Planning cost moves up front; reviews compare code against an approved spec.

## Alternatives considered
- GitHub Spec Kit — heavier per-feature ceremony, weaker brownfield story.
- Plain ADRs + issues — no living behaviour baseline.
