# frozen_string_literal: true

# Production-readiness feature gates.
#
# These features are built but not yet considered production-ready, so they ship
# OFF by default — in both cloud and self-hosted builds — and are opt-in via ENV
# (set the var to "1" to enable, e.g. for dogfooding). This mirrors the existing
# `ENABLE_… == "1"` convention. When a feature graduates, delete its gate here
# and the call sites that read it.
#
# This is a different axis from the per-workspace billing gates
# (`Entitlements` / `config/plans.yml`): those answer "is this workspace's plan
# allowed to use the feature?", whereas these are global, deploy-level switches
# answering "is this feature ready to be shown to anyone at all?". A feature can
# be entitlement-gated *and* readiness-gated; both must pass.
#
# Defined as a plain module (not just an ApplicationController `helper_method`)
# so it can be read from anywhere — controllers, views, Phlex components,
# background jobs and service objects alike. The workflow ingress
# (WorkflowTriggerJob, the event bus, the public webhook) runs outside the
# request cycle, where a `helper_method` would not reach.
module Features
  class << self
    # The Workflow engine: builder UI, triggers, the public webhook ingress and
    # the public API. Gated end-to-end (UI + ingress) so nothing fires until it
    # is enabled — see WorkflowsController, WebhooksController, EmailProcessJob
    # and Events::Publisher.
    def workflows?
      flag?("ENABLE_WORKFLOWS")
    end

    # The inbox "Board" (status kanban) layout. When off, Default and List remain
    # the available inbox layouts (see Campbooks::InboxViewSwitcher).
    def email_board?
      flag?("ENABLE_EMAIL_BOARD")
    end

    # AI-generated, reusable document templates: the Settings → Document templates
    # manager plus the fill/preview/send surfaces. Gated until production-ready;
    # orthogonal to the per-workspace `:document_templates` billing entitlement.
    def document_templates?
      flag?("ENABLE_DOCUMENT_TEMPLATES")
    end

    # AI-drafted, reusable email templates: the Settings → Email templates manager,
    # the compose-time picker, and the public API. Gated until production-ready;
    # orthogonal to the per-workspace `:email_templates` billing entitlement.
    def email_templates?
      flag?("ENABLE_EMAIL_TEMPLATES")
    end

    # Every Microsoft 365 surface: "Sign in with Microsoft", mailbox connect, the
    # OAuth callbacks and the Settings → Security link/unlink. Honors the legacy
    # ENABLE_MICROSOFT_MAILBOX so deployments that already enabled mailbox connect
    # keep working after the flags were unified.
    def microsoft?
      flag?("ENABLE_MICROSOFT") || flag?("ENABLE_MICROSOFT_MAILBOX")
    end

    # The Tasks module: AI task extraction, the task board/skim/feed surfaces,
    # Scout + Cmd+K email linking, and the public API. Gated end-to-end (nav, web,
    # API, extraction jobs) until production-ready. Orthogonal to the per-workspace
    # `:tasks` billing entitlement — both must pass.
    def tasks?
      flag?("ENABLE_TASKS")
    end

    # User-configured scheduled digests: saved scopes + rrule + optional AI
    # summarization, delivered by email and/or home feed. Gated until production-
    # ready; orthogonal to the per-workspace `:digests` billing entitlement.
    def digests?
      flag?("ENABLE_DIGESTS")
    end

    # The Accounting module: bank-statement reconciliation against Documents,
    # CSV parsing, transaction matching, and zip export. Gated end-to-end (nav,
    # web, jobs) until production-ready. Orthogonal to the per-workspace
    # `:accounting` billing entitlement — both must pass.
    def accounting?
      flag?("ENABLE_ACCOUNTING")
    end

    private

    def flag?(name)
      ENV[name] == "1"
    end
  end
end
