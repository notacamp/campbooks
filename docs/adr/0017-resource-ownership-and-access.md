# 0017 — Resource ownership and access: one catalogue, one check

- Status: accepted
- Date: 2026-10-14
- Tracking: notacamp/campbooks-cloud#117 (security audit remediation)
- Change: `openspec/changes/access-guards`

## Context

The 2026-10-13 security audit found no critical hole, but it found access
decided in many places, each a little differently. Each module had its own
helpers (`connections::visible`, `visible_inboxes`, `lock_conversation`,
`require_sender`, `acting_manager`), the change feed and search asked a
visibility port, and the idempotency layer asked nobody. Nothing listed who
owns what. Nothing failed when a new handler forgot to check. Some symptoms:

- a workspace admin could disconnect a member's mailbox;
- a removed member kept receiving a stream's ids until reconnect;
- connection and draft changes reached every member;
- a stored response could be replayed to a caller who had since lost
  access.

The owner asked for **one explicit authorization model**: resources are
reachable only by their owners and by whoever they are shared with. It has
to be declared once, checked centrally, and enforced by CI.

## Decision

1. **A resource catalogue.** Every module declares its resource types in its
   descriptor (`Module.resources`, `kernel::authz::ResourceType`). Each type
   names its **tenancy** (workspace, account, instance, public) and its
   **owner** (the member who created or connected it, the workspace's owners,
   the account, or nobody). It lists the **actions** the owner takes (`read`,
   `write`, `delete`, `send`, `share`, `disconnect`) and its **sharing**:
   grants of actions to a workspace role, to a role in the resource's
   container (an inbox's `reader` < `sender` < `manager`), to the owner
   while they hold a container role, to whoever may see a container of
   another module (an attention item in an inbox, answered by the
   `Visibility` port), to client tokens (explicit and narrower), or to
   anyone (public types). A client a member registered acts for that member
   (`ClientDirectory` port, answered by the issuer): it gets their access,
   no more, and stops working (401) once they leave or the client is
   removed. Inbox and inbox-contained types grant nothing to clients as
   such. With an outside identity provider (no issuer in this process)
   nothing answers the port, so every client token is "unregistered": it
   reads no inbox, but the explicit `Clients` grants on the workspace, its
   members and its connections still apply. With the issuer in process,
   a client token it does not know as a workspace's own is refused (401). The kernel builds one catalogue
   from all modules (`Registry::resources`, installed in `AppState`) and
   rejects duplicate or misnamed types and unknown container roles.
2. **One central check**, `authz::authorize(state, tx, access, action,
   resource) -> Allow | NotFound | Forbidden`. It is deny by default. The
   owning module answers only the **facts** of one instance through its
   `ResourceFacts` port: does it exist in this workspace, who owns it, which
   container is it in, what is the caller's role there, is it frozen
   read-only. The kernel applies the catalogue. Seeing is `read`: a caller
   who may not read a resource gets `NotFound` whatever the action, and
   `Forbidden` is reserved for visible resources. `authorize_type`
   answers for lists and creations, and `authorize_account` for a person's
   own account resources. An allowed resource inside a container is noted
   for the idempotency layer, so a replay is served only while the caller
   may still see it.
3. **Row-level security stays the second layer.** The kernel's own
   workspace-keyed tables now carry it too: the change feed, the blob
   catalogue, the event log and the queue. A workspace transaction cannot
   read or write another workspace's kernel rows. `EventBus::publish`
   refuses an event of another workspace than its transaction's, and a
   workspace event in a transaction with no workspace set. A transaction
   with no workspace set still sees every workspace (the kernel's sweeps
   rely on it); making that an explicit kernel scope is
   campbooks-cloud#129. *(Since done, by the `kernel-scope` change: a
   transaction that sets neither a workspace nor the kernel scope sees no
   row, and cross-workspace work opens the scope through
   `kernel::db::begin_kernel` or a job's `kernel_tx`, outside the kernel
   only in files `architecture.toml` lists under `[[access.kernel_scope]]`.)*
4. **Enforcement.** Every operation declares `x-campbooks.access =
   {resource, action}`. `cargo xtask arch` fails:
   - an undeclared operation, or one naming an unknown type or an action
     nobody holds;
   - a workspace or account operation whose handler never calls the check
     (or an allowlisted helper that does);
   - module code querying a workspace table on the pool or in a `.begin()`
     transaction (reviewed exceptions in `architecture.toml`);
   - a published event with no consumer and no stated reason.

   A test generated from the catalogue checks every workspace resource type
   × principal × action against the module's real facts. Another, generated
   from `openapi.json`, calls every operation as every kind of caller.

### The owner's decisions, encoded

- A connection is owned by the member who connected it. Only they hold
  `disconnect`: owners and admins see it (and may sync it) but get 403
  `disconnect_owner_only`.
- Workspace owners and admins get **nothing** inside an inbox they are not
  a member of. No inbox or inbox-contained type grants anything to a
  workspace role.
- Disconnecting deletes the mail by default. `?mail=keep` keeps it
  read-only, and only the owner can choose that, being the only one who
  disconnects. When a member is removed, an owner or admin may choose
  `?mail=keep` for the mailboxes that member connected, unless an inbox
  would then have nobody left to read it (409 `keep_needs_a_reader`,
  answered by the `KeptMail` port). A member who leaves on their own keeps
  nothing. A kept inbox's facts say `frozen`, and the check then refuses
  every action but `read` and `delete`. Its scheduled sends go back to
  drafts, and a send released later re-checks `send`.
- A member who left rejoins only once every module has handled their
  departure (409 `still_leaving`): `workspaces.member_left` must settle
  (`DomainEvent::SETTLES`), so a delivery of it that fails for good is never
  pruned and keeps blocking until an operator replays or discards it.
- A disconnect and a sync batch never interleave: removing or freezing an
  inbox locks it first, and a batch holds it `FOR SHARE` and is dropped
  when it finds the inbox closed.

## Alternatives considered

1. **Per-module checks (what we had).** Each module writes its own
   helpers. Nothing new to build, but nothing lists owners or sharing. The
   rules drift (three of the four audit findings were drift), and CI cannot
   tell a forgotten check from a deliberate one.
2. **RLS only.** Push every rule into Postgres policies (membership tables
   joined in policies). It is enforced even against SQL injection, but it
   cannot express owner and role rules across modules without cross-schema
   joins (forbidden by ADR 0002). It turns every denial into an empty
   result, so a read cannot be told from a forbidden write. And the policies
   would need the user and role in session settings for every query. We
   keep RLS for tenancy, which it is good at.
3. **A policy engine (Cedar, Oso/Polar, OpenFGA).** These give a mature
   language and analysis tools. The cost is a new dependency, and with
   OpenFGA a new service for self-hosters to run, against ADR 0006's one
   image. There would be a second language for contributors, and the
   entities still have to be loaded per module, which is the same facts
   port we built. Our model is small: owner, roles, containers, clients.
   It fits in a few hundred lines of Rust that the compiler and our tests
   check. If it outgrows that (custom roles, per-record sharing lists), the
   catalogue maps directly onto Cedar's schema and the facts port onto
   entity loading, so we can switch without touching handlers.
4. **The chosen design: a declarative catalogue plus a central check, with
   module-provided facts.** The rules live in one place per module and are
   readable, testable and enforceable. Modules keep their data and
   language. The cost is a port implementation per module (one query per
   type) and one extra query per checked request.

Not copied from the Rails app on `main`. That app used per-controller
`before_action` checks and model scopes, which is alternative 1.

## Consequences

- A new resource type cannot ship without declaring its owner, sharing and
  client grant. A new operation cannot ship without declaring what it does
  to which type, and calling the check with that type (by name, by a
  constant holding it, or through a helper listed for it).
- The identity module's account resources, the web BFF's session and proxy
  operations, and workspace-bound third-party tokens are registered in the
  catalogue. Their per-instance facts move onto the check in a follow-up
  change (PR B of campbooks-cloud#117). Until then their handlers call
  `require_account`, or are listed as pending with the issue.
- Lists still filter with module queries (`visible_inboxes`) that must agree
  with the catalogue. The matrix test checks that they do.
