# Workflow Actions — Registry, Integrations & the Email Bridge

> Status: **design spec** for incremental work by multiple agents. Companion to `docs/action-system.md`,
> which covers the *inbound* email-action registry (`EmailActions`). This doc covers the *outbound*
> workflow-action system and how the two connect. Nothing here is built yet — it is the backlog.

## 1. The problem

The app has **two** systems called "actions." They model opposite things and share **zero** code
(grep comes back empty in both directions — `Workflows::Executor` never references `EmailActions`/`Tools::*`,
and `EmailActions` never references `Workflow*`):

| | `EmailActions` (`app/services/email_actions.rb`) | Workflow actions (`WorkflowStep` + `Workflows::Executor`) |
|---|---|---|
| **Direction** | **Inbound** — mutate an existing email (archive, tag, snooze) | **Outbound** — emit a side-effect (send mail, POST, Slack/Discord) |
| **Target** | a specific `EmailMessage` | a `TriggerContext` — there is no record to mutate |
| **Actor** | a **user**, gated by `accessible_by?` / `sendable_by?` | the **workspace**, background, no acting user |
| **Config** | runtime `args` hash, literal | persisted `config` jsonb, **Liquid-templated** |
| **Safety** | per-user permission checks | `UrlGuard` SSRF blocking + `HttpClient` size/time caps |
| **Registration** | data-driven `DEFINITIONS` array — add one entry | code sprawl across **6 files** |

The inbound side already proves the better pattern (one `Definition`, one `.run`). The outbound side is the
one with the sprawl: adding a workflow action type today means editing **six** places in lockstep —

1. `WorkflowStep::ACTION_TYPES` (+ `ACTION_LABELS`, + `HTTP_ACTION_TYPES` if HTTP-backed)
2. `Workflows::Executor#execute_action` dispatch `when` branch
3. `Workflows::Executor#build_*_request` builder method
4. `Campbooks::StepPicker::CATALOG` card
5. `Campbooks::WorkflowStepForm` — `ACTION_OPTIONS` + `panel(...)` + a `*_fields` method
6. `WorkflowsController#workflow_params` — config keys merged into one flat permit list

## 2. Vision

**One registry _pattern_, two registries, bridged at exactly one point.**

- `EmailActions` — inbound. Keep as is (it already is the pattern).
- `Workflows::ActionRegistry` — outbound. **New.** Mirror the `EmailActions` struct, plus a declarative
  `config_schema` that drives the builder form, strong params, Liquid rendering, and validation from one place.
- **Bridge:** a single `email_action` workflow step delegates into `EmailActions.run`, so an automation can act
  on its triggering email. This reuses the inbound registry rather than duplicating it.

**Non-goal: a single shared table of definitions.** The two shapes are incompatible — inbound actions carry
`surfaces`/`destructive`/`perm` (for Cmd+K/Scout) and take literal args; outbound actions carry a Liquid
`config_schema` and run with no user. Merging them yields a struct full of nil fields and `if` branches. They
share an *interface idea*, not a row.

## 3. Layer 0 — `Workflows::ActionRegistry` (foundation; behavior-preserving)

A registry mirroring `EmailActions`, with one new field — `config_schema` — that collapses the 6-file sprawl.

```ruby
# app/services/workflows/action_registry.rb
Definition = Struct.new(
  :key,            # "send_email", "http_request", "custom_action", "email_action"
  :label,          # "Send Email"
  :group,          # :messaging | :http | :email | :logic  (StepPicker grouping)
  :icon,           # :mail
  :description,
  :config_schema,  # [ { key:, type:, label:, options:, hint: }, ... ]  ← drives everything
  :builder,        # ->(config, renderer) { { method:, url:, headers:, body: } }   (HTTP family)
  :executor,       # ->(config, context, step_execution) { ... }                   (send_email, email_action)
  keyword_init: true
)
```

The `config_schema` is a declarative field list. Field `type`s map to form widgets **and** to how `Executor`
renders them through Liquid:

| `type` | Builder widget | Liquid-rendered? |
|---|---|---|
| `:select` | `<select>` (needs `options:`) | no |
| `:string` | plain text input | no |
| `:liquid` | single-line `LiquidField` | yes |
| `:liquid_textarea` | multi-line `LiquidField` | yes |
| `:liquid_lines` | textarea, `Key: Value` per line | yes (each value) |
| `:account_select` | sendable-account picker | no |
| `:integration_select` | integration picker (Layer 1) | no |

From **one** definition the system generates all six of today's edit points:

- **`StepPicker::CATALOG`** → `{ group, key, title: label, icon, description }`
- **`WorkflowStepForm`** panel → loop `config_schema` → `liquid_field` / `select` per field
- **`workflow_params`** → permit list = `registry.flat_map { _1.config_schema.map(&:key) }.uniq`
- **`Executor` Liquid pass** → render every `:liquid*` field before dispatch
- **`ACTION_TYPES` / `ACTION_LABELS`** → derived from registry keys/labels (model validates against the registry)
- **`HTTP_ACTION_TYPES`** → definitions that define a `builder`

`Executor#execute_action` collapses to:

```ruby
defn   = Workflows::ActionRegistry.definition(step.action_type)
config = render_liquid(defn.config_schema, step.config)   # only :liquid* fields
if defn.builder
  execute_http(defn.builder.call(config, @renderer), step_execution)   # → HttpClient + UrlGuard
else
  defn.executor.call(config, @context, step_execution)                 # send_email, email_action
end
```

`send_email` keeps its bespoke executor (re-scope account to workspace → `account.mail_client.send_message` →
record sent `EmailMessage`). The HTTP family (`http_request`/`slack_message`/`discord_message`) keeps riding
`execute_http` → `HttpClient`/`UrlGuard`. **No behavior change** — characterization-test the current
`Executor.call` outputs first, then refactor under green.

## 4. Layer 1 — `Integration` model + `custom_action`

New workspace-scoped model, modelled on `EmailAccount`:

```
Integration
  workspace_id
  name            # "Stripe (prod)"
  base_url        # "https://api.stripe.com"
  auth_type       # none | bearer | header | basic
  auth_secret     # encrypted via ActiveRecord::Encryption (like EmailAccount#zoho_refresh_token)
  default_headers # jsonb, optional
```

CRUD at `/integrations` (model the controller/views on `/email_accounts`).

`custom_action` is a registry definition whose `builder` resolves the integration **server-side** — injects
`base_url` + the auth header, then merges the step's Liquid `path`/`headers`/`body` — and hands the result to
the existing `execute_http`. So it rides the same `HttpClient`/`UrlGuard` rails; the only new thing is that
**secrets never appear in plaintext Liquid** (today you'd paste a token into each step's `headers`).

`config_schema`: `integration_id` (`:integration_select`), `http_method` (`:select`), `path` (`:liquid`),
`headers` (`:liquid_lines`), `body` (`:liquid_textarea`).

## 5. Layer 1b — "nicer raw HTTP" falls out for free

Once the form is schema-driven and auth presets exist, the existing `http_request` step *is* `custom_action`
with an inline, unsaved connection. Add auth-preset fields + an optional **"Test"** button (one `HttpClient`
call, result shown inline) and `http_request` inherits the better UX with no extra registry entry. The two may
be collapsed into one definition that toggles `integration_id` vs. inline auth.

## 6. Layer 2 — `email_action` (the bridge; the actual merge)

A registry definition whose `executor` pulls the triggering email from `EmailContext#email` and calls
`EmailActions.run(tool, email_message:, args:, user:)`. This **reuses** the inbound registry — no duplication.

- Add a new `:workflow` surface to the `EmailActions` `DEFINITIONS` so only safe actions are exposable in
  automations (tag/archive/snooze — **not** `delete`). The builder's action dropdown is generated from
  `EmailActions.tools_for(:workflow)`.
- `config_schema`: `email_tool` (`:select`, options from the registry), plus the chosen tool's args
  (e.g. `tag_name`) as `:liquid` fields.

**The one real decision — the actor.** `EmailActions.run` requires `user:`; workflows have no
`Current.acting_user`. Options:

| Option | Verdict |
|---|---|
| **(ii) Workflow owner/creator as the actor** | **Recommended.** Slots straight into the existing `accessible_by?`/`sendable_by?` gates and the `Current.acting_user` work in the email-permissions project. The automation acts "as" whoever built it. No new principal. |
| (i) Per-workspace system/bot user | Cleaner long-term, auditable, but a new concept to introduce and thread through permissions. Defer. |
| (iii) Workspace-scoped bypass for automations | Rejected — fights the permissions model; an automation could touch accounts its author can't. |

## 7. Build order & acceptance criteria

**Order:** 0 → 1 → (1b free) → 2. Layer 0 is behavior-preserving and unblocks everything; Layer 2 is small
once the actor is chosen.

**Acceptance criteria (mirrors `docs/action-system.md` §8):**

- Adding a workflow action type requires editing **one** registry entry; it then appears in the StepPicker, the
  step form, strong params, and the executor automatically.
- Layer 0 ships with **no behavior change** — the existing workflow specs stay green, and the builder UI renders
  identically (verify at mobile width per `CLAUDE.md`).
- `custom_action` never exposes a secret in rendered Liquid or execution logs.
- `email_action` can only run actions flagged for the `:workflow` surface, gated by the workflow owner's
  permissions; it can never run a destructive action (`delete`).
