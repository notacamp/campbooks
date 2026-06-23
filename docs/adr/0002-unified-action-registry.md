---
status: accepted
---

# A single global Action registry, consumed by every surface

## Context

Actions on the product grew up per-domain and per-surface. `EmailActions` (`app/services/email_actions.rb`) is a real canonical registry — but it is **email-only** and **mutation-only**, and only some surfaces route through it (Scout's two paths do; `BulkController`, Cmd+K, and Skim still carry their own `case tool` ladders — see `docs/action-system.md`). The ~25 document actions live entirely in `DocumentsController` / `Documents::SkimController` and are reachable from **no** shared surface — Cmd+K has two navigation links and Scout has only the read-only `query_documents`. Calendar and settings actions are likewise controller-bound. Meanwhile `Workflows::ActionRegistry` is a *third* thing called "actions" (workflow step types), and its `email_action` step already bridges into `EmailActions`.

The result: the same intent runs through different code on different surfaces, the AI can only act on email, and "Action" means three different things. The goal is to **centralise every user action so Cmd+K and Scout can take any of them**, with typed targets so an email can't be fed to a document action.

## Decision

One **global Action registry** owns every *user operation on a resource* across email, documents, calendar, and settings. Each Action declares, declaratively, everything a surface needs:

- **Kind** — `mutation` | `query` | `navigation`.
- **Target** — the resource *type* it acts on (or none, for a Global/settings Action). A user may only run it against a Target they can access, so an email can never be passed to a document Action.
- **Cardinality** — one or many; a collection arrives from multi-select **or** from a **Target Projection** (email → its attached documents, thread → its messages, sender → their messages). Projection is permission-filtered and runs per-item with an aggregated, skip-and-report result. This makes "bulk" and "fan-out" one mechanism, and one verb (`archive`) one entry rather than `archive` + `bulk_archive`.
- **Level** — required permission, `read` | `send` | `manage`, checked against the acting user's per-resource flags (the existing `can_read`/`can_send`/`can_manage` on `EmailAccountUser`/`CalendarAccountUser`; a new `DocumentUser` for documents; `User#admin?` for workspace-scoped Global Actions).
- **Group** + **Scout Autonomy ceiling** — the family it belongs to, and the *most* autonomy Scout may ever have for it (`forbidden` / `suggest` / `auto`). A workspace setting dials the *actual* Scout autonomy per Group within that ceiling.
- **Arguments** — a typed schema, reused to validate input, render Cmd+K forms, describe the tool to Scout, and render workflow-step config.

All surfaces — the email/document UIs, Cmd+K, Scout, Skim, and workflow **action-steps** — become thin adapters over `Actions.run(...)` and generate their catalogs from `Actions.available_for(...)`. None re-implement execution. **`EmailActions` is evolved in place to become the email slice** of this registry; document/calendar/settings slices are built out against the same shape.

Workflows keep their own **Step** taxonomy — *trigger*, *action*, *control-flow*, *integration effect* — and only *action*-steps draw from the registry. Triggers, control-flow, and the HTTP/Slack/Discord/custom effects stay workflow-only and never appear in Cmd+K or Scout. `Workflows::ActionRegistry` accordingly shrinks toward the non-action step types, and its `email_action` bridge generalises to any resource Action.

## Considered Options

- **Per-domain registries** (`EmailActions` + `DocumentActions` + `CalendarActions`…). Rejected: re-creates today's divergence one level up — Cmd+K and Scout would have to integrate N registries, and cross-resource fan-out (email → its documents) would have nowhere to live.
- **Keep the registry mutation-only; reads & navigation stay separate.** Rejected: Cmd+K is mostly navigation and Scout leans on read tools, so both would remain multi-sourced. Modelling them as `query`/`navigation` Kinds makes the registry the single catalog.
- **Fold workflow triggers/control-flow/effects into the registry too.** Rejected: they aren't operations on a workspace resource and would pollute the human/AI action surfaces with automation plumbing.
- **Binary resource access instead of Levels.** Rejected: it would drop the view-vs-send distinction email already enforces (`accessible_by?` vs `sendable_by?`) — a viewer of a shared mailbox could send mail as the owner.
- **A hardcoded "AI never touches destructive/settings" line.** Rejected in favour of a per-Group, ceiling-capped **workspace setting**, because the product will widen unsupervised AI over time and that policy must be configurable, not baked in.

## Consequences

- **Email** refactors in place and low-risk (`EmailActions` specs already exist); the win is that `BulkController`, Cmd+K, and Skim stop carrying their own dispatch ladders.
- **New building blocks** are required: a `DocumentUser` sharing model (documents **default to shared with every workspace member**, the model only narrows/elevates — so no regression); a declared **Target Projection** graph; the **Kind / Group / Autonomy / Arguments** schema on the Action definition; and a workspace **Autonomy Policy** setting (itself an `admin?`-gated Global Action).
- **`surfaces` sheds its `scout_*` entries** — Scout exposure becomes *derived* from `(autonomy ceiling + workspace policy)` rather than hardcoded per action.
- **Scout and Cmd+K become generated** from the registry: adding one Action entry exposes it (where permitted) on every surface at once — the acceptance criterion from `docs/action-system.md`, now generalised beyond email.
- Calendar and settings action inventories still need to be taken before their slices can be written (email and documents are inventoried).
