# Campbooks

Campbooks ingests a workspace's emails and documents, uses AI ("Scout") to classify and surface what needs attention, and gives the user a review-and-approval workflow. This glossary fixes the language the product and the code should share.

## Language

**Workspace**:
The top-level container that owns all documents, emails, settings, members, and AI configuration. Every record belongs to exactly one; there is one per account.
_Avoid_: Organization, Company, Account, Tenant

**Workspace Type**:
Whether a Workspace represents a Company or an Individual. "Company" is therefore a *type of* Workspace, never a synonym for one.
_Avoid_: using "Company" to mean the Workspace itself

**Setup**:
The steps a Workspace finishes before Campbooks is fully useful (workspace profile, email account, AI provider, document types, tags). Incomplete steps surface as the setup banner and its modals; `SetupStatus` is the source of truth.
_Avoid_: Wizard

**Onboarding**:
The full-page, first-run flow for a brand-new Workspace. Distinct from Setup: critical/warning Setup tasks redirect into Onboarding, while info tasks stay in the banner.

**Setup Task**:
A single incomplete Setup step (e.g. "Add document types"), each with a severity. Critical and warning tasks redirect to Onboarding; info tasks only show the banner.

**Scout**:
The product's AI assistant persona, the single name users see for classification, document analysis, drafting, and chat. A brand voice, not a model.

**Document Type**:
What a document fundamentally *is* (invoice, receipt, contract, bank statement). Drives what Scout extracts. A document has exactly one.
_Avoid_: Category, Label

**Tag**:
A cross-cutting label applied to emails by topic, project, or priority; an email can have many. Scout reads the tag's description to auto-classify. Structurally similar to a Document Type but a distinct concept: Types answer "what kind of document is this?", Tags answer "what is this email about, and how urgent?".
_Avoid_: Category; for documents use Document Type

**AI Adapter**:
A configured connection to one AI provider: provider name + API key (encrypted) + optional endpoint URL.
_Avoid_: Provider (that is only the vendor), Integration

**AI Configuration**:
The binding of an AI Adapter to a Purpose, with the model and generation params to use. An enabled AI Configuration is what marks the AI Setup Task complete.
_Avoid_: AI settings, Model config

**Purpose**:
The job an AI Configuration serves: `email_classification` (text) or `document_analysis` (needs a vision-capable provider: OpenAI, Anthropic, or Gemini; not DeepSeek).

## Actions

**Action**:
A named operation a user can take on the product — archive an email, approve a document, change a setting. Defined once in a single canonical registry that declares what the Action needs to run (its Kind, Target, Cardinality, typed Arguments, required Level, and Group) and how to execute it; every Surface runs the *same* definition.
_Avoid_: Tool (the underlying runner an Action wraps), Command, Workflow Step (an Action may be *invoked by* a Step, but is not one).

**Surface**:
A place an Action can be invoked from — the email UI (single and bulk), the Cmd+K palette, Scout, Skim, the workflow builder. An Action declares which Surfaces expose it; surfaces are thin adapters that never re-implement execution.

**Target**:
The resource an Action operates on, named by type (email message, thread, sender, document, calendar event…). An Action declares its Target type, and a user may run the Action only on a Target they can access.
_Avoid_: Subject, Object.

**Cardinality**:
How many Targets one Action run touches — a single resource or a collection. The same verb (e.g. Archive) is one Action carrying a Cardinality, not two separate single/bulk Actions. A collection of Targets arises either from a user's multi-select or from a Target Projection.
_Avoid_: Bulk as its own kind of action.

**Global Action**:
An Action with no resource Target — it acts on the Workspace as a whole (or the acting User's own settings) rather than a specific email or document. Settings changes are Global Actions.
_Avoid_: inventing a placeholder Target for them.

**Permission Level**:
The minimum capability an Action requires of the acting user on its Target, from a fixed ladder: `read` (view + non-outbound change), `send` (act outward as the resource), `manage` (destructive or configuration change). For shared email and calendar resources these map to the per-user flags `can_read`/`can_send`/`can_manage` (roles viewer/collaborator/manager, plus owner). A user lacking the required Level on the Target cannot run the Action. Documents carry the same per-user sharing as email and calendar, except they **default to being shared with every member of the Workspace** (email and calendar default to the owner plus explicit grants); the per-user model only narrows or elevates from that default, and destructive document Actions still require `manage`. Global Actions that need elevation check the Workspace-wide `admin` role on User.

**Target Projection**:
A declared mapping from one resource type to a set of related resources of another type — an email to its attached documents, a thread to its messages, a sender to their messages. It lets an Action that targets the second type run from a handle on the first ("approve every document attached to this email"). The projected set is permission-filtered before the Action runs once per surviving Target, and partial results are reported (skip-and-report, not atomic-fail). Projection and multi-select are the two ways one Action run reaches many Targets.
_Avoid_: modelling fan-out as a distinct kind of Action.

**Action Kind**:
What an Action *does* to the system: `mutation` (changes a resource — carries a Level and a destructive flag), `query` (read-only; access-filtered, never destructive), or `navigation` (moves the user somewhere; no Target, no permission). Cmd+K surfaces all three; Scout sees mutations and queries.

**Action Group**:
A named family of related Actions (email triage, document review, classification, calendar, settings…). Groups organise the Cmd+K and Scout catalogs and are the default unit at which Scout Autonomy is configured.

**Scout Autonomy**:
How far Scout may go with an Action on its own. Each Action declares a *ceiling* it can never exceed — `forbidden` (never available to Scout; Cmd+K / human only, e.g. settings changes), `suggest` (Scout may propose, a human approves), or `auto` (Scout may run it unsupervised). Within that ceiling a Workspace setting configures the *actual* level Scout uses, set per Action Group, with per-Action overrides for groups flagged granular (e.g. email). Today only classification/extraction runs `auto`; everything else is `suggest`. Workflows are authorised separately — by the human who built them — so a workflow step is not bound by this ceiling.
_Avoid_: treating "unsupervised" as one global switch.

**Workflow Step**:
A unit in a Workflow's orchestration graph, of one type: a *trigger* (what starts the run), an *action* (which invokes a registry Action on the trigger's resource, or a Target Projection of it), a *control-flow* step (branch, filter, loop, delay), or an *integration effect* (an outbound HTTP / Slack / Discord / custom call with no resource Target). Only action-steps draw from the global Action registry; triggers, control-flow, and effects are workflow-only and never appear in Cmd+K or Scout.
_Avoid_: calling a Step an Action — a Step may invoke an Action, but is not one.
_Avoid_: equating resource `manage` with the Workspace-wide `admin` role on User — they are distinct.
